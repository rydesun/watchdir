use std::{
    collections::HashMap,
    ffi::CString,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use snafu::Snafu;
use tracing::info;
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

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Failed to use inotify API"))]
    InotifyInit,

    #[snafu(display("Failed to watch: {}", path.display()))]
    InotifyAdd { path: PathBuf },

    #[snafu(display("Duplicated watch: {}", path.display()))]
    InotifyAddDup { wd: i32, path: PathBuf },
}

type Result<T, E = Error> = std::result::Result<T, E>;

type Cookie = u32;

pub struct Watcher {
    opts: WatcherOpts,
    fd: i32,
    wds: HashMap<i32, PathBuf>,
    event_seq: EventSeq,
    prev: Option<(EventKind, Cookie, PathBuf)>,
    cached_inotify_event: Option<inotify::Event>,
    cached_events: Option<Box<dyn Iterator<Item = Event>>>,
}

#[derive(Copy, Clone)]
struct WatcherOpts {
    sub_dotdir: Dotdir,
}

impl Watcher {
    pub fn new<I>(dirs: I, sub_dotdir: Dotdir) -> Result<Self>
    where
        I: IntoIterator,
        I::Item: AsRef<Path>,
    {
        let fd = unsafe { libc::inotify_init() };
        if fd < 0 {
            return Err(Error::InotifyInit);
        }
        let event_seq = EventSeq::new(fd);

        let mut watcher = Self {
            fd,
            opts: WatcherOpts { sub_dotdir },
            wds: HashMap::new(),
            event_seq,
            prev: None,
            cached_inotify_event: None,
            cached_events: None,
        };
        for d in dirs {
            watcher.add_all_watch(d.as_ref());
        }

        Ok(watcher)
    }

    fn add_watch(&mut self, path: &Path) -> Result<()> {
        let ffi_path = CString::new(path.as_os_str().as_bytes()).unwrap();
        let event_types = libc::IN_CREATE | libc::IN_MOVE | libc::IN_MOVE_SELF;
        let wd = unsafe {
            libc::inotify_add_watch(self.fd, ffi_path.as_ptr(), event_types)
        };
        if wd < 0 {
            return Err(Error::InotifyAdd { path: path.to_owned() });
        }
        if self.wds.insert(wd, path.to_owned()) == None {
            Ok(())
        } else {
            Err(Error::InotifyAddDup { wd, path: path.to_owned() })
        }
    }

    fn add_all_watch(&mut self, d: &Path) -> Vec<PathBuf> {
        if let Err(e) = self.add_watch(d) {
            info!("{}", e);
        }
        let opts = self.opts;
        let mut new_dirs = Vec::new();

        WalkDir::new(d)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| guard(opts, e.path()))
            .filter_map(Result::ok)
            .for_each(|e| {
                let dir = e.path();
                if let Err(e) = self.add_watch(dir) {
                    info!("{}", e);
                } else {
                    new_dirs.push(dir.to_owned());
                }
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
        let inotify_event = self
            .cached_inotify_event
            .take()
            .unwrap_or_else(|| self.event_seq.next().unwrap());

        if let Some((kind, cookie, path)) = self.prev.take() {
            if matches!(inotify_event.kind, EventKind::MoveTo) {
                if inotify_event.cookie != cookie {
                    self.cached_inotify_event = Some(inotify_event);
                    // FIXME: undo watching
                    return Some(Event::MoveAway(path));
                }
                let full_path = self.get_full_path(
                    inotify_event.wd,
                    &inotify_event.path.unwrap(),
                );
                return Some(Event::Move(path.to_owned(), full_path));
            } else if matches!(inotify_event.kind, EventKind::MoveSelf) {
                if matches!(kind, EventKind::MoveFrom) {
                    // FIXME: undo watching
                    return Some(Event::MoveAway(path));
                } else {
                    // FIXME: update watched subdirs
                    *self.wds.get_mut(&inotify_event.wd).unwrap() =
                        path.to_owned();
                    return self.next();
                }
            } else {
                self.cached_inotify_event = Some(inotify_event);
                // FIXME: undo watching
                return Some(Event::MoveAway(path));
            }
        }

        match inotify_event.kind {
            EventKind::Create => {
                let full_path = self.get_full_path(
                    inotify_event.wd,
                    &inotify_event.path.unwrap(),
                );
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
                let full_path = self.get_full_path(
                    inotify_event.wd,
                    &inotify_event.path.unwrap(),
                );
                self.prev = Some((
                    EventKind::MoveFrom,
                    inotify_event.cookie,
                    full_path,
                ));
                // FIXME: too laggy for file moving
                self.next()
            }
            EventKind::MoveTo => {
                let full_path = self.get_full_path(
                    inotify_event.wd,
                    &inotify_event.path.unwrap(),
                );
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
    // FIXME: metadata is unreliable
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
    use std::fs::{create_dir, create_dir_all, rename, File};

    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    use super::*;

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
        let mut watcher =
            Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

        let path = top_dir.path().join(random_string(5));
        File::create(&path).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Create(path))
    }

    #[test]
    fn test_create_in_created_subdir() {
        let top_dir = tempfile::tempdir().unwrap();
        let mut watcher =
            Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

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
        let mut watcher =
            Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

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

    #[test]
    fn test_move_dir() {
        let top_dir = tempfile::tempdir().unwrap();
        let old_dir = top_dir.path().join(random_string(5));
        create_dir(&old_dir).unwrap();

        let mut watcher =
            Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

        let new_dir = top_dir.path().join(random_string(5));
        rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();

        assert_eq!(watcher.next().unwrap(), Event::Move(old_dir, new_dir))
    }

    #[test]
    fn test_move_file() {
        let top_dir = tempfile::tempdir().unwrap();
        let old_file = top_dir.path().join(random_string(5));
        File::create(&old_file).unwrap();

        let mut watcher =
            Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

        let new_file = top_dir.path().join(random_string(5));
        rename(old_file.to_owned(), new_file.to_owned()).unwrap();

        assert_eq!(watcher.next().unwrap(), Event::Move(old_file, new_file))
    }

    #[test]
    fn test_dir_move_away() {
        let top_dir = tempfile::tempdir().unwrap();
        let unwatched_dir = tempfile::tempdir().unwrap();
        let old_dir = top_dir.path().join(random_string(5));
        create_dir(&old_dir).unwrap();

        let mut watcher =
            Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

        let new_dir = unwatched_dir.path().join(random_string(5));
        rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();

        assert_eq!(watcher.next().unwrap(), Event::MoveAway(old_dir))
    }

    #[test]
    fn test_file_move_away() {
        let top_dir = tempfile::tempdir().unwrap();
        let unwatched_dir = tempfile::tempdir().unwrap();
        let old_file = top_dir.path().join(random_string(5));
        File::create(&old_file).unwrap();

        let mut watcher =
            Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

        let new_file = unwatched_dir.path().join(random_string(5));
        rename(old_file.to_owned(), new_file.to_owned()).unwrap();

        // FIXME: It is waiting for next event
        let next_file = top_dir.path().join(random_string(5));
        File::create(&next_file).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::MoveAway(old_file));
        assert_eq!(watcher.next().unwrap(), Event::Create(next_file))
    }

    #[test]
    fn test_dir_move_into() {
        let top_dir = tempfile::tempdir().unwrap();
        let unwatched_dir = tempfile::tempdir().unwrap();
        let old_dir = unwatched_dir.path().join(random_string(5));
        create_dir(&old_dir).unwrap();

        let mut watcher =
            Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

        let new_dir = top_dir.path().join(random_string(5));
        rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();

        assert_eq!(watcher.next().unwrap(), Event::MoveInto(new_dir))
    }

    #[test]
    fn test_file_move_info() {
        let top_dir = tempfile::tempdir().unwrap();
        let unwatched_dir = tempfile::tempdir().unwrap();
        let old_file = unwatched_dir.path().join(random_string(5));
        File::create(&old_file).unwrap();

        let mut watcher =
            Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

        let new_file = top_dir.path().join(random_string(5));
        rename(old_file.to_owned(), new_file.to_owned()).unwrap();

        assert_eq!(watcher.next().unwrap(), Event::MoveInto(new_file));
    }

    #[test]
    fn test_file_move_away_and_move_into() {
        let top_dir = tempfile::tempdir().unwrap();
        let unwatched_dir = tempfile::tempdir().unwrap();

        let old_file = top_dir.path().join(random_string(5));
        File::create(&old_file).unwrap();

        let next_file_name = random_string(5);
        let next_old_file =
            unwatched_dir.path().join(next_file_name.to_owned());
        File::create(&next_old_file).unwrap();

        let mut watcher =
            Watcher::new(vec![&top_dir], Dotdir::Exclude).unwrap();

        let new_file = unwatched_dir.path().join(random_string(5));
        rename(old_file.to_owned(), new_file.to_owned()).unwrap();
        let next_new_file = top_dir.path().join(next_file_name);
        rename(next_old_file.to_owned(), next_new_file.to_owned()).unwrap();

        assert_eq!(watcher.next().unwrap(), Event::MoveAway(old_file));
        assert_eq!(watcher.next().unwrap(), Event::MoveInto(next_new_file))
    }
}
