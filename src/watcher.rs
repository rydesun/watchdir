use std::{
    collections::HashMap,
    ffi::CString,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use walkdir::WalkDir;

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
    opts: WatcherOpts,
    fd: i32,
    wds: HashMap<i32, PathBuf>,
    cookie: Option<(u32, PathBuf)>,
    event_seq: EventSeq,
    cached_event: Option<(inotify::RawEvent, inotify::Event)>,
    cached_events: Option<Box<dyn Iterator<Item = Event>>>,
}

#[derive(Copy, Clone)]
struct WatcherOpts {
    sub_dotdir: Dotdir,
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
            opts: WatcherOpts { sub_dotdir },
            wds: HashMap::new(),
            cookie: None,
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
        self.add_watch(d);
        let opts = self.opts;
        let mut new_dirs = Vec::new();

        WalkDir::new(d)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| guard(opts, e.path()))
            .filter_map(Result::ok)
            .for_each(|e| {
                let dir = e.path();
                self.add_watch(dir);
                new_dirs.push(dir.to_owned());
            });

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
                if guard(self.opts, &full_path) {
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
                if guard(self.opts, &full_path) {
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

fn guard(opts: WatcherOpts, path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    if path.file_name().unwrap().as_bytes()[0] == b'.' {
        matches!(opts.sub_dotdir, Dotdir::Include)
    } else {
        true
    }
}
