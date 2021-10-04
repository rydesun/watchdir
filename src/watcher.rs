use std::{
    collections::HashMap,
    ffi::CString,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use walkdir::{DirEntry, WalkDir};

use crate::inotify;
use crate::inotify::{EventKind, EventSeq};

#[derive(Debug)]
pub enum Event {
    Create(PathBuf),
    Move(PathBuf, PathBuf),
    MoveAway(PathBuf),
    MoveInto(PathBuf),
    // TODO
    // Delete(PathBuf),
    Unknown,
}

#[derive(Copy, Clone)]
pub enum Dotdir {
    Include,
    Exclude,
}

pub struct Watcher {
    fd: i32,
    wds: HashMap<i32, PathBuf>,
    cookie: Option<(u32, PathBuf)>,
    sub_dotdir: Dotdir,
    event_seq: EventSeq,
    cached_event: Option<(inotify::RawEvent, inotify::Event)>,
    cached_events: Option<Box<dyn Iterator<Item = Event>>>,
}

impl Watcher {
    pub fn new<I>(dirs: I, sub_dotdir: Dotdir) -> Result<Self, String>
    where
        I: IntoIterator,
        I::Item: AsRef<Path>,
    {
        let fd = unsafe { libc::inotify_init() };
        if fd < 0 {
            return Err("Failed to use inotify API".to_owned());
        }
        let event_seq = EventSeq::new(fd);

        let mut watcher = Self {
            fd,
            wds: HashMap::new(),
            cookie: None,
            sub_dotdir,
            event_seq,
            cached_event: None,
            cached_events: None,
        };
        for d in dirs {
            watcher.add_all_watch(d.as_ref());
        }

        Ok(watcher)
    }

    fn add_watch(&mut self, path: &Path) {
        let ffi_path = CString::new(path.as_os_str().as_bytes()).unwrap();
        let event_types = libc::IN_CREATE | libc::IN_MOVE;
        let wd = unsafe { libc::inotify_add_watch(self.fd, ffi_path.as_ptr(), event_types) };
        self.wds.insert(wd, path.to_owned());
    }

    fn add_all_watch(&mut self, d: &Path) -> Vec<PathBuf> {
        let mut new_dirs = Vec::new();
        let sub_dotdir = self.sub_dotdir;

        let walker = WalkDir::new(d)
            .into_iter()
            .filter_entry(|e| {
                e.depth() > 0 || !(matches!(sub_dotdir, Dotdir::Exclude) && is_hidden(e))
            })
            .filter_map(Result::ok);

        for entry in walker {
            let path = entry.path();
            if path.is_dir() {
                self.add_watch(path);
                if entry.depth() > 0 {
                    new_dirs.push(path.to_owned());
                }
            }
        }
        new_dirs
    }

    fn get_full_path(&self, wd: i32, path: &Path) -> PathBuf {
        self.wds[&wd].to_owned().join(path)
    }
}

impl Iterator for Watcher {
    type Item = Event;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(cached_events) = &mut self.cached_events {
            if let Some(event) = cached_events.next() {
                return Some(event);
            }
        }
        let (raw_event, event) = self
            .cached_event
            .take()
            .unwrap_or_else(|| self.event_seq.next().unwrap());

        if let Some(cookie_pair) = &self.cookie.take() {
            if matches!(event.kind, EventKind::MoveTo) && raw_event.cookie == cookie_pair.0 {
                let full_path = self.get_full_path(raw_event.wd, &event.path);
                // FIXME: update watched name
                return Some(Event::Move(cookie_pair.1.to_owned(), full_path));
            }

            self.cached_event = Some((raw_event, event));
            // FIXME: undo watching
            return Some(Event::MoveAway(cookie_pair.1.to_owned()));
        }

        let full_path = self.get_full_path(raw_event.wd, &event.path);
        match event.kind {
            EventKind::Create => {
                if full_path.is_dir() {
                    self.cached_events = Some(Box::new(
                        self.add_all_watch(&full_path)
                            .into_iter()
                            .map(|d| Event::Create(d)),
                    ));
                }
                Some(Event::Create(full_path))
            }
            EventKind::MoveFrom => {
                self.cookie = Some((raw_event.cookie, full_path));
                self.next()
            }
            EventKind::MoveTo => {
                if full_path.is_dir() {
                    self.add_all_watch(&full_path);
                }
                Some(Event::MoveInto(full_path))
            }
            _ => Some(Event::Unknown),
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        for wd in self.wds.keys() {
            unsafe { libc::inotify_rm_watch(self.fd, *wd) };
        }
    }
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}
