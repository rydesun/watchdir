mod inotify;
mod path_tree;

use std::{
    ffi::CString,
    fs::{self, FileType},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use snafu::Snafu;
use tracing::warn;
use walkdir::WalkDir;

#[derive(PartialEq, Debug)]
pub enum Event {
    Create(PathBuf),
    MoveDir(PathBuf, PathBuf),
    MoveFile(PathBuf, PathBuf),
    MoveAwayDir(PathBuf),
    MoveAwayFile(PathBuf),
    MoveInto(PathBuf),
    MoveTop(PathBuf),
    DeleteDir(PathBuf),
    DeleteFile(PathBuf),
    DeleteTop(PathBuf),
    Modify(PathBuf),
    Ignored,
    Unknown,
}

#[derive(Copy, Clone)]
pub enum Dotdir {
    Include,
    Exclude,
}

impl From<bool> for Dotdir {
    fn from(v: bool) -> Self {
        if v {
            Self::Include
        } else {
            Self::Exclude
        }
    }
}

#[derive(Debug, Snafu)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    #[snafu(display("Failed to use inotify API"))]
    InitInotify,

    #[snafu(display("Failed to watch: {}: {}", path.display(), source))]
    AddWatch { source: std::io::Error, path: PathBuf },

    #[snafu(display("Watch the same path multiple times: {}", path.display()))]
    WatchSame { wd: i32, path: PathBuf },
}

type Result<T, E = Error> = std::result::Result<T, E>;

pub struct Watcher {
    opts: WatcherOpts,
    fd: i32,
    top_wd: i32,
    top_dir: PathBuf,
    path_tree: path_tree::Head<i32>,
    event_seq: inotify::EventSeq,
    cached_inotify_event: Option<inotify::Event>,
    cached_events: Option<Box<dyn Iterator<Item = Event>>>,
}

#[derive(Copy, Clone)]
pub struct WatcherOpts {
    sub_dotdir: Dotdir,
    event_types: u32,
}

impl WatcherOpts {
    pub fn new(sub_dotdir: Dotdir, modify_event: bool) -> Self {
        let mut event_types = libc::IN_CREATE
            | libc::IN_MOVE
            | libc::IN_MOVE_SELF
            | libc::IN_DELETE
            | libc::IN_DELETE_SELF;
        if modify_event {
            event_types |= libc::IN_MODIFY;
        }

        Self { sub_dotdir, event_types }
    }
}

impl Watcher {
    pub fn new(dir: &Path, opts: WatcherOpts) -> Result<Self> {
        let fd = unsafe { libc::inotify_init() };
        if fd < 0 {
            return Err(Error::InitInotify);
        }
        let event_seq = inotify::EventSeq::new(fd);

        let mut watcher = Self {
            fd,
            opts,
            top_wd: 0,
            top_dir: dir.to_owned(),
            path_tree: path_tree::Head::new(dir.to_owned()),
            event_seq,
            cached_inotify_event: None,
            cached_events: None,
        };
        if let (Some(top_wd), _) = watcher.add_watch_all(dir) {
            watcher.top_wd = top_wd;
        }

        Ok(watcher)
    }

    fn add_watch(&mut self, path: &Path) -> Result<i32> {
        let ffi_path = CString::new(path.as_os_str().as_bytes()).unwrap();
        let wd = unsafe {
            libc::inotify_add_watch(
                self.fd,
                ffi_path.as_ptr(),
                self.opts.event_types,
            )
        };
        if wd < 0 {
            return Err(Error::AddWatch {
                source: std::io::Error::last_os_error(),
                path: path.to_owned(),
            });
        }

        self.path_tree.insert(path, wd).unwrap();
        Ok(wd)
    }

    fn add_watch_all(&mut self, d: &Path) -> (Option<i32>, Vec<PathBuf>) {
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
            .filter_entry(|e| guard(opts, e.path(), e.file_type()))
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

    fn full_path(&self, wd: i32, path: &Path) -> PathBuf {
        self.path_tree.full_path(wd, path)
    }

    fn update_path(&mut self, wd: i32, path: &Path) {
        self.path_tree.rename(wd, path).unwrap()
    }

    fn rm_watch_all(&mut self, wd: i32) {
        let values = self.path_tree.delete(wd).unwrap();
        for wd in values {
            unsafe {
                libc::inotify_rm_watch(self.fd, wd);
            }
        }
    }

    fn next_inotify_event(&mut self) -> Option<inotify::Event> {
        if self.event_seq.has_next_event() {
            Some(self.event_seq.next().unwrap())
        } else {
            None
        }
    }

    fn recognize(
        &mut self,
        inotify_event: inotify::Event,
    ) -> (Event, Option<i32>) {
        let wd = inotify_event.wd;

        match inotify_event.kind {
            inotify::EventKind::Create(path) => {
                let full_path = self.full_path(wd, &path);
                (Event::Create(full_path), None)
            }

            inotify::EventKind::MoveFrom(from_path) => {
                let full_from_path = self.full_path(wd, &from_path);
                if let Some(next_inotify_event) = self.next_inotify_event() {
                    match next_inotify_event.kind {
                        inotify::EventKind::MoveSelf => {
                            if next_inotify_event.wd != self.top_wd {
                                (
                                    Event::MoveAwayDir(full_from_path),
                                    Some(next_inotify_event.wd),
                                )
                            } else {
                                self.cached_inotify_event =
                                    Some(next_inotify_event);
                                (Event::MoveAwayFile(full_from_path), None)
                            }
                        }
                        inotify::EventKind::MoveTo(ref to_path) => {
                            if inotify_event.cookie
                                != next_inotify_event.cookie
                            {
                                self.cached_inotify_event =
                                    Some(next_inotify_event);
                                (Event::MoveAwayFile(full_from_path), None)
                            } else {
                                let full_to_path = self
                                    .full_path(next_inotify_event.wd, to_path);
                                if let Some(next2_inotify_event) =
                                    self.next_inotify_event()
                                {
                                    match next2_inotify_event.kind {
                                        inotify::EventKind::MoveSelf => (
                                            Event::MoveDir(
                                                full_from_path,
                                                full_to_path,
                                            ),
                                            Some(next2_inotify_event.wd),
                                        ),
                                        _ => {
                                            self.cached_inotify_event =
                                                Some(next2_inotify_event);
                                            (
                                                Event::MoveFile(
                                                    full_from_path,
                                                    full_to_path,
                                                ),
                                                None,
                                            )
                                        }
                                    }
                                } else {
                                    (
                                        Event::MoveFile(
                                            full_from_path,
                                            full_to_path,
                                        ),
                                        None,
                                    )
                                }
                            }
                        }
                        _ => {
                            self.cached_inotify_event =
                                Some(next_inotify_event);
                            (Event::MoveAwayFile(full_from_path), None)
                        }
                    }
                } else {
                    (Event::MoveAwayFile(full_from_path), None)
                }
            }

            inotify::EventKind::MoveTo(path) => {
                let full_path = self.full_path(wd, &path);
                (Event::MoveInto(full_path), None)
            }

            inotify::EventKind::Delete(path) => {
                let full_path = self.full_path(wd, &path);
                if let Some(next_inotify_event) = self.next_inotify_event() {
                    match next_inotify_event.kind {
                        inotify::EventKind::DeleteSelf => {
                            if next_inotify_event.wd == self.top_wd {
                                self.cached_inotify_event =
                                    Some(next_inotify_event);
                                (Event::DeleteFile(full_path), None)
                            } else {
                                (
                                    Event::DeleteDir(full_path),
                                    Some(next_inotify_event.wd),
                                )
                            }
                        }
                        _ => {
                            self.cached_inotify_event =
                                Some(next_inotify_event);
                            (Event::DeleteFile(full_path), None)
                        }
                    }
                } else {
                    (Event::DeleteFile(full_path), None)
                }
            }

            inotify::EventKind::MoveSelf => {
                (Event::MoveTop(self.top_dir.to_owned()), None)
            }

            inotify::EventKind::DeleteSelf => {
                (Event::DeleteTop(self.top_dir.to_owned()), None)
            }

            inotify::EventKind::Modify(path) => {
                let full_path = self.full_path(wd, &path);
                (Event::Modify(full_path), None)
            }

            inotify::EventKind::Ignored => (Event::Ignored, None),
            _ => (Event::Unknown, None),
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

        let (event, wd) = self.recognize(inotify_event);

        match event {
            Event::MoveDir(_, ref path) => {
                self.update_path(wd.unwrap(), path);
            }
            Event::MoveAwayDir(_) | Event::DeleteDir(_) => {
                self.rm_watch_all(wd.unwrap());
            }
            Event::MoveInto(ref path) => {
                if let Ok(metadata) = fs::symlink_metadata(path) {
                    if guard(self.opts, path, metadata.file_type()) {
                        self.add_watch_all(path);
                    }
                }
            }
            Event::Create(ref path) => {
                if let Ok(metadata) = fs::symlink_metadata(path) {
                    if guard(self.opts, path, metadata.file_type()) {
                        self.cached_events = Some(Box::new(
                            self.add_watch_all(path)
                                .1
                                .into_iter()
                                .map(Event::Create),
                        ));
                    }
                }
            }
            Event::DeleteTop(_) => {
                self.rm_watch_all(self.top_wd);
            }

            _ => {}
        }

        Some(event)
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        for wd in self.path_tree.values() {
            unsafe { libc::inotify_rm_watch(self.fd, *wd) };
        }
    }
}

fn guard(opts: WatcherOpts, path: &Path, file_type: FileType) -> bool {
    if !file_type.is_dir() {
        return false;
    }
    if path.file_name().unwrap().as_bytes()[0] == b'.' {
        matches!(opts.sub_dotdir, Dotdir::Include)
    } else {
        true
    }
}