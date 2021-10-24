use std::{
    ffi::CString,
    fs::{self, Metadata},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use snafu::Snafu;
use tracing::warn;
use walkdir::WalkDir;

use crate::inotify;
use crate::inotify::{EventKind, EventSeq};
use crate::path_tree;

#[derive(PartialEq, Debug)]
pub enum Event {
    Create(PathBuf),
    Move(PathBuf, PathBuf),
    MoveAway(PathBuf),
    MoveInto(PathBuf),
    MoveTop,
    Delete(PathBuf),
    Modify(PathBuf),
    DeleteTop,
    Ignored,
    Unknown,
}

#[derive(Copy, Clone)]
pub enum Dotdir {
    Include,
    Exclude,
}

#[derive(Debug, Snafu)]
#[allow(clippy::enum_variant_names)]
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
    top_wd: i32,
    path_tree: path_tree::Head<i32>,
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
    pub fn new(dir: &Path, sub_dotdir: Dotdir) -> Result<Self> {
        let fd = unsafe { libc::inotify_init() };
        if fd < 0 {
            return Err(Error::InotifyInit);
        }
        let event_seq = EventSeq::new(fd);

        let mut watcher = Self {
            fd,
            opts: WatcherOpts { sub_dotdir },
            top_wd: 0,
            path_tree: path_tree::Head::new(dir.to_owned()),
            event_seq,
            prev: None,
            cached_inotify_event: None,
            cached_events: None,
        };
        if let (Some(top_wd), _) = watcher.add_all_watch(dir) {
            watcher.top_wd = top_wd;
        }

        Ok(watcher)
    }

    fn add_watch(&mut self, path: &Path) -> Result<i32> {
        let ffi_path = CString::new(path.as_os_str().as_bytes()).unwrap();
        let event_types = libc::IN_CREATE
            | libc::IN_MOVE
            | libc::IN_MOVE_SELF
            | libc::IN_DELETE
            | libc::IN_DELETE_SELF
            | libc::IN_MODIFY;
        let wd = unsafe {
            libc::inotify_add_watch(self.fd, ffi_path.as_ptr(), event_types)
        };
        if wd < 0 {
            return Err(Error::InotifyAdd { path: path.to_owned() });
        }

        self.path_tree.insert(path, wd);
        Ok(wd)
    }

    fn add_all_watch(&mut self, d: &Path) -> (Option<i32>, Vec<PathBuf>) {
        let top_wd = match self.add_watch(d) {
            Err(e) => {
                warn!("{}", e);
                None
            }
            Ok(wd) => Some(wd),
        };
        let opts = self.opts;
        let mut new_dirs = Vec::new();

        WalkDir::new(d)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| {
                if let Ok(metadata) = e.metadata() {
                    guard(opts, e.path(), metadata)
                } else {
                    false
                }
            })
            .filter_map(Result::ok)
            .for_each(|e| {
                let dir = e.path();
                if let Err(e) = self.add_watch(dir) {
                    warn!("{}", e);
                } else {
                    new_dirs.push(dir.to_owned());
                }
            });

        (top_wd, new_dirs)
    }

    fn get_full_path(&self, wd: i32, path: &Path) -> PathBuf {
        self.path_tree.get_full_path(wd, path)
    }

    fn update_path(&mut self, wd: i32, path: &Path) {
        self.path_tree.rename(wd, path)
    }

    fn unwatch_all(&mut self, wd: i32) {
        let values = self.path_tree.delete(wd);
        for wd in values {
            unsafe {
                libc::inotify_rm_watch(self.fd, wd);
            }
        }
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
                    return Some(Event::MoveAway(path));
                }
                let full_path = self.get_full_path(
                    inotify_event.wd,
                    &inotify_event.path.unwrap(),
                );
                self.prev = Some((
                    EventKind::MoveTo,
                    inotify_event.cookie,
                    full_path.to_owned(),
                ));
                return Some(Event::Move(path.to_owned(), full_path));
            } else if matches!(inotify_event.kind, EventKind::MoveSelf) {
                if matches!(kind, EventKind::MoveFrom) {
                    self.unwatch_all(inotify_event.wd);
                    return Some(Event::MoveAway(path));
                } else {
                    self.update_path(inotify_event.wd, &path);
                    return self.next();
                }
            } else if matches!(kind, EventKind::MoveFrom) {
                self.cached_inotify_event = Some(inotify_event);
                return Some(Event::MoveAway(path));
            }
        }

        match inotify_event.kind {
            EventKind::Create => {
                let full_path = self.get_full_path(
                    inotify_event.wd,
                    &inotify_event.path.unwrap(),
                );
                if let Ok(metadata) = fs::symlink_metadata(&full_path) {
                    if guard(self.opts, &full_path, metadata) {
                        self.cached_events = Some(Box::new(
                            self.add_all_watch(&full_path)
                                .1
                                .into_iter()
                                .map(Event::Create),
                        ));
                    }
                }
                Some(Event::Create(full_path))
            }
            EventKind::MoveFrom => {
                let full_path = self.get_full_path(
                    inotify_event.wd,
                    &inotify_event.path.unwrap(),
                );
                if self.event_seq.has_next_event() {
                    self.prev = Some((
                        EventKind::MoveFrom,
                        inotify_event.cookie,
                        full_path,
                    ));
                    self.next()
                } else {
                    Some(Event::MoveAway(full_path))
                }
            }
            EventKind::MoveTo => {
                let full_path = self.get_full_path(
                    inotify_event.wd,
                    &inotify_event.path.unwrap(),
                );
                if let Ok(metadata) = fs::symlink_metadata(&full_path) {
                    if guard(self.opts, &full_path, metadata) {
                        self.add_all_watch(&full_path);
                    }
                }
                Some(Event::MoveInto(full_path))
            }
            EventKind::Delete => {
                let full_path = self.get_full_path(
                    inotify_event.wd,
                    &inotify_event.path.unwrap(),
                );
                Some(Event::Delete(full_path))
            }
            EventKind::DeleteSelf => {
                self.unwatch_all(inotify_event.wd);
                if inotify_event.wd == self.top_wd {
                    Some(Event::DeleteTop)
                } else {
                    self.next()
                }
            }
            EventKind::MoveSelf => {
                if inotify_event.wd == self.top_wd {
                    Some(Event::MoveTop)
                } else {
                    Some(Event::Unknown)
                }
            }
            EventKind::Modify => {
                let full_path = self.get_full_path(
                    inotify_event.wd,
                    &inotify_event.path.unwrap(),
                );
                Some(Event::Modify(full_path))
            }
            EventKind::Ignored => Some(Event::Ignored),
            _ => Some(Event::Unknown),
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        for wd in self.path_tree.values() {
            unsafe { libc::inotify_rm_watch(self.fd, *wd) };
        }
    }
}

fn guard(opts: WatcherOpts, path: &Path, metadata: Metadata) -> bool {
    // FIXME: metadata is unreliable
    if !metadata.is_dir() {
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
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        let path = top_dir.path().join(random_string(5));
        File::create(&path).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Create(path))
    }

    #[test]
    fn test_create_in_created_subdir() {
        let top_dir = tempfile::tempdir().unwrap();
        let mut watcher =
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        let dir = top_dir.path().join(random_string(5));
        create_dir(&dir).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Create(dir.to_owned()));

        let path = dir.join(random_string(5));
        File::create(&path).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Create(path))
    }

    #[test]
    fn test_create_in_recur_created_subdir() {
        let top_dir = tempfile::tempdir().unwrap();
        let mut watcher =
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        let recur_depth = 3;
        let mut dir = top_dir.path().to_owned();
        let mut dirs: Vec<PathBuf> = Vec::<PathBuf>::new();
        for _ in 0..recur_depth {
            dir = dir.join(random_string(5));
            dirs.push(dir.to_owned());
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
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        let new_dir = top_dir.path().join(random_string(5));
        rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();

        assert_eq!(watcher.next().unwrap(), Event::Move(old_dir, new_dir))
    }

    #[test]
    fn test_create_in_moved_subdir() {
        let top_dir = tempfile::tempdir().unwrap();

        let old_dir = top_dir.path().join(random_string(5));

        let mut sub_dirs = PathBuf::new();
        for _ in 0..3 {
            sub_dirs.push(PathBuf::from(random_string(5)));
        }
        create_dir_all(&old_dir.join(sub_dirs.to_owned())).unwrap();

        let mut watcher =
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        let new_dir = top_dir.path().join(random_string(5));

        rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();
        assert_eq!(
            watcher.next().unwrap(),
            Event::Move(old_dir, new_dir.to_owned())
        );

        let new_file = new_dir.join(sub_dirs).join(random_string(5));
        File::create(&new_file).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Create(new_file))
    }

    #[test]
    fn test_create_in_moved_dir_in_subdir() {
        let top_dir = tempfile::tempdir().unwrap();

        let old_dir = top_dir.path().join(random_string(5));
        create_dir(&old_dir).unwrap();

        let mut sub_dirs = top_dir.path().to_owned();
        for _ in 0..3 {
            sub_dirs.push(PathBuf::from(random_string(5)));
        }
        create_dir_all(&top_dir.path().join(sub_dirs.to_owned())).unwrap();

        let mut watcher =
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        let new_dir = sub_dirs.to_owned().join(random_string(5));
        rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();
        assert_eq!(
            watcher.next().unwrap(),
            Event::Move(old_dir, new_dir.to_owned())
        );

        let new_file = new_dir.join(random_string(5));
        File::create(&new_file).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Create(new_file))
    }

    #[test]
    fn test_move_file() {
        let top_dir = tempfile::tempdir().unwrap();
        let old_file = top_dir.path().join(random_string(5));
        File::create(&old_file).unwrap();

        let mut watcher =
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

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
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        let new_dir = unwatched_dir.path().join(random_string(5));
        rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();

        assert_eq!(watcher.next().unwrap(), Event::MoveAway(old_dir));

        let unwatched_file = new_dir.join(random_string(5));
        File::create(&unwatched_file).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Ignored);
    }

    #[test]
    fn test_file_move_away() {
        let top_dir = tempfile::tempdir().unwrap();
        let unwatched_dir = tempfile::tempdir().unwrap();
        let old_file = top_dir.path().join(random_string(5));
        File::create(&old_file).unwrap();

        let mut watcher =
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        let new_file = unwatched_dir.path().join(random_string(5));
        rename(old_file.to_owned(), new_file).unwrap();

        assert_eq!(watcher.next().unwrap(), Event::MoveAway(old_file));
    }

    #[test]
    fn test_dir_move_into() {
        let top_dir = tempfile::tempdir().unwrap();
        let unwatched_dir = tempfile::tempdir().unwrap();
        let old_dir = unwatched_dir.path().join(random_string(5));
        create_dir(&old_dir).unwrap();

        let mut watcher =
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        let new_dir = top_dir.path().join(random_string(5));
        rename(old_dir, new_dir.to_owned()).unwrap();

        assert_eq!(
            watcher.next().unwrap(),
            Event::MoveInto(new_dir.to_owned())
        );

        let new_file = new_dir.join(random_string(5));
        File::create(&new_file).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Create(new_file));
    }

    #[test]
    fn test_file_move_into() {
        let top_dir = tempfile::tempdir().unwrap();
        let unwatched_dir = tempfile::tempdir().unwrap();
        let old_file = unwatched_dir.path().join(random_string(5));
        File::create(&old_file).unwrap();

        let mut watcher =
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        let new_file = top_dir.path().join(random_string(5));
        rename(old_file, new_file.to_owned()).unwrap();

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
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        let new_file = unwatched_dir.path().join(random_string(5));
        rename(old_file.to_owned(), new_file).unwrap();
        let next_new_file = top_dir.path().join(next_file_name);
        rename(next_old_file, next_new_file.to_owned()).unwrap();

        assert_eq!(watcher.next().unwrap(), Event::MoveAway(old_file));
        assert_eq!(watcher.next().unwrap(), Event::MoveInto(next_new_file))
    }

    #[test]
    fn test_remove_file() {
        let top_dir = tempfile::tempdir().unwrap();

        let path = top_dir.path().join(random_string(5));
        File::create(&path).unwrap();

        let mut watcher =
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        fs::remove_file(&path).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Delete(path))
    }

    #[test]
    fn test_remove_dir() {
        let top_dir = tempfile::tempdir().unwrap();

        let dir = top_dir.path().join(random_string(5));
        fs::create_dir(&dir).unwrap();

        let mut watcher =
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        fs::remove_dir(&dir).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Delete(dir))
    }

    #[test]
    fn test_remove_dir_recursively() {
        let top_dir = tempfile::tempdir().unwrap();

        let dir = top_dir.path().to_owned().join(random_string(5));
        let mut sub_dir = dir.to_owned();
        for _ in 0..3 {
            sub_dir.push(random_string(5));
        }
        create_dir_all(&sub_dir).unwrap();
        let file = sub_dir.join(random_string(5));
        File::create(&file).unwrap();

        let mut watcher =
            Watcher::new(top_dir.as_ref(), Dotdir::Exclude).unwrap();

        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(watcher.next().unwrap(), Event::Delete(file));

        for _ in 0..3 {
            assert_eq!(
                watcher.next().unwrap(),
                Event::Delete(sub_dir.to_owned())
            );
            assert_eq!(watcher.next().unwrap(), Event::Ignored,);
            sub_dir.pop();
        }
    }
}
