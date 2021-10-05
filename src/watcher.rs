use std::{
    collections::HashMap,
    ffi::CString,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use walkdir::WalkDir;

use crate::inotify;
use crate::inotify::{EventKind, EventSeq};

#[derive(PartialEq, Debug)]
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
                            .map(Event::Create),
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    use std::fs::{create_dir, create_dir_all, File};

    fn random_string(len: usize) -> String {
        thread_rng()
            .sample_iter(&Alphanumeric)
            .take(len)
            .map(char::from)
            .collect()
    }

    #[test]
    fn test_create_file() {
        let top_dir = tempfile::tempdir().unwrap();
        let mut watcher = Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

        let path = top_dir.path().join(random_string(5));
        File::create(&path).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Create(path))
    }

    #[test]
    fn test_create_in_created_subdir() {
        let top_dir = tempfile::tempdir().unwrap();
        let mut watcher = Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

        let dir = top_dir.path().join(random_string(5));
        create_dir(&dir).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Create(dir.clone()));

        let path = dir.join(random_string(5));
        File::create(&path).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Create(path))
    }

    #[test]
    fn test_create_in_recur_created_subdir() {
        let top_dir = tempfile::tempdir().unwrap();
        let mut watcher = Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

        let recur_depth = 3;
        let mut dir = top_dir.path().to_owned();
        let mut dirs: Vec<PathBuf> = Vec::<PathBuf>::new();
        for _ in 0..recur_depth {
            dir = dir.join(random_string(5));
            dirs.push(dir.clone());
        }
        create_dir_all(&dir).unwrap();
        for d in dirs.iter().take(recur_depth) {
            assert_eq!(watcher.next().unwrap(), Event::Create(d.to_owned()));
        }

        let path = dir.join(random_string(5));
        File::create(&path).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Create(path))
    }
}
