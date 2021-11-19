mod inotify;
mod path_tree;

use std::{
    ffi::CString,
    fs,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use async_stream::stream;
use futures::{pin_mut, Stream, StreamExt};
use snafu::Snafu;
use tracing::warn;
use walkdir::WalkDir;

#[derive(PartialEq, Debug)]
pub enum Event {
    Create(PathBuf, FileType),
    Move(PathBuf, PathBuf, FileType),
    MoveAway(PathBuf, FileType),
    MoveInto(PathBuf, FileType),
    MoveTop(PathBuf),
    Delete(PathBuf, FileType),
    DeleteTop(PathBuf),
    Modify(PathBuf, FileType),
    Access(PathBuf, FileType),
    AccessTop(PathBuf),
    Attrib(PathBuf, FileType),
    AttribTop(PathBuf),
    Open(PathBuf, FileType),
    OpenTop(PathBuf),
    Close(PathBuf, FileType),
    CloseTop(PathBuf),
    Unmount(PathBuf, FileType),
    UnmountTop(PathBuf),
    Noise,
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
    InitInotify,

    #[snafu(display("{}: {}", source, path.display()))]
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
}

#[derive(Copy, Clone)]
pub struct WatcherOpts {
    sub_dotdir: Dotdir,
    event_types: u32,
}

impl WatcherOpts {
    pub fn new(sub_dotdir: Dotdir, extra_events: Vec<ExtraEvent>) -> Self {
        let mut event_types = libc::IN_CREATE
            | libc::IN_MOVE
            | libc::IN_MOVE_SELF
            | libc::IN_DELETE
            | libc::IN_DELETE_SELF
            | libc::IN_ONLYDIR;
        event_types = extra_events.iter().fold(event_types, |v, e| match e {
            ExtraEvent::Modify => v | libc::IN_MODIFY,
            ExtraEvent::Access => v | libc::IN_ACCESS,
            ExtraEvent::Attrib => v | libc::IN_ATTRIB,
            ExtraEvent::Open => v | libc::IN_OPEN,
            ExtraEvent::Close => v | libc::IN_CLOSE,
        });

        Self { sub_dotdir, event_types }
    }
}

pub enum ExtraEvent {
    Modify,
    Access,
    Attrib,
    Open,
    Close,
}

impl Watcher {
    pub fn new(dir: &Path, opts: WatcherOpts) -> Result<Self> {
        let fd = unsafe { libc::inotify_init() };
        if fd < 0 {
            return Err(Error::InitInotify);
        }

        let mut watcher = Self {
            fd,
            opts,
            top_wd: 0,
            top_dir: dir.to_owned(),
            path_tree: path_tree::Head::new(dir.to_owned()),
            event_seq: inotify::EventSeq::new(fd),
            cached_inotify_event: None,
        };
        if let (Some(top_wd), walk) = watcher.add_watch_all(dir) {
            watcher.top_wd = top_wd;
            for entry in walk {
                if let Err(e) = watcher.add_watch(entry.path()) {
                    warn!("{}", e);
                }
            }
        }

        Ok(watcher)
    }

    pub fn stream(
        &mut self,
    ) -> impl Stream<Item = (Event, time::OffsetDateTime)> + '_ {
        stream! {
            loop {
                let (inotify_event, event, wd) = loop {
                    let inotify_event = match self.cached_inotify_event.take()
                    {
                        Some(e) => e,
                        None => {
                            let stream = self.event_seq.stream();
                            pin_mut!(stream);
                            // FIXME: handle error
                            stream.next().await.unwrap().unwrap()
                        }
                    };
                    let (event, wd) = self.recognize(&inotify_event).await;
                    if event != Event::Noise {
                        break (inotify_event, event, wd);
                    }
                };

                match event {
                    Event::Move(ref from_path, ref to_path, FileType::Dir) => {
                        if guard(self.opts, from_path, FileType::Dir) {
                            if guard(self.opts, to_path, FileType::Dir) {
                                self.update_path(wd.unwrap(), to_path);
                            } else {
                                self.rm_watch_all(wd.unwrap());
                            }
                        } else {
                            if guard(self.opts, to_path, FileType::Dir) {
                                let (_, walk) = self.add_watch_all(to_path);
                                for entry in walk {
                                    if let Err(e) = self.add_watch(
                                        entry.path()) {
                                        warn!("{}", e);
                                    }
                                }
                            }
                        }
                        yield (event, inotify_event.t)
                    }
                    Event::MoveAway(_, FileType::Dir)
                        | Event::Delete(_, FileType::Dir) => {
                        if let Some(wd) = wd {
                            self.rm_watch_all(wd);
                        }
                        yield (event, inotify_event.t)
                    }
                    Event::MoveInto(ref path, FileType::Dir) => {
                        if let Ok(metadata) = fs::symlink_metadata(path) {
                            if guard(self.opts, path,
                                metadata.file_type().into()) {
                                let (_, walk) = self.add_watch_all(path);
                                for entry in walk {
                                    if let Err(e) = self.add_watch(
                                        entry.path()) {
                                        warn!("{}", e);
                                    }
                                }
                            }
                        }
                        yield (event, inotify_event.t)
                    }
                    Event::Create(ref path, FileType::Dir) => {
                        if let Ok(metadata) = fs::symlink_metadata(path) {
                            if guard(self.opts, path,
                                metadata.file_type().into()) {
                                let next_events: Vec<Event> = self
                                    .add_watch_all(path)
                                    .1
                                    .map(|entry| entry.path().to_owned())
                                    .map(|path| {
                                        if let Err(e) = self.add_watch(&path) {
                                            warn!("{}", e);
                                        }
                                        path
                                    })
                                    .map(|path| Event::Create(
                                            path, FileType::Dir))
                                    .collect();

                                yield (event, inotify_event.t);
                                for event in next_events {
                                    yield (event, inotify_event.t)
                                }
                            } else {
                                yield (event, inotify_event.t)
                            }
                        } else {
                            yield (event, inotify_event.t)
                        }
                    }
                    Event::DeleteTop(_) | Event::UnmountTop(_) => {
                        let top_wd = self.top_wd;
                        self.rm_watch_all(top_wd);
                        yield (event, inotify_event.t)
                    }
                    Event::Unmount(..) => {
                        self.rm_watch_all(inotify_event.wd);
                        yield (event, inotify_event.t)
                    }

                    _ => {
                        yield (event, inotify_event.t)
                    }
                }
            }
        }
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

        if self.path_tree.has(wd) {
            return Err(Error::WatchSame { wd, path: path.to_owned() });
        }

        self.path_tree.insert(path, wd).unwrap();
        Ok(wd)
    }

    fn add_watch_all(
        &mut self,
        path: &Path,
    ) -> (Option<i32>, impl Iterator<Item = walkdir::DirEntry>) {
        let top_wd = match self.add_watch(path) {
            Err(e) => {
                warn!("{}", e);
                None
            }
            Ok(wd) => Some(wd),
        };
        let opts = self.opts;
        let new_dirs = WalkDir::new(path)
            .min_depth(1)
            .into_iter()
            .filter_entry(move |entry| {
                guard(opts, entry.path(), entry.file_type().into())
            })
            .filter_map(Result::ok);

        (top_wd, new_dirs)
    }

    fn path(&self, wd: i32) -> PathBuf {
        self.path_tree.path(wd)
    }

    fn full_path(&self, wd: i32, path: &Path) -> PathBuf {
        self.path(wd).join(path)
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

    async fn next_inotify_event(&mut self) -> Option<inotify::Event> {
        if self.event_seq.has_next_event() {
            let stream = self.event_seq.stream();
            pin_mut!(stream);
            // FIXME: handle error
            Some(stream.next().await.unwrap().unwrap())
        } else {
            None
        }
    }

    pub fn has_next_event(&mut self) -> bool {
        self.cached_inotify_event.is_some() | self.event_seq.has_next_event()
    }

    async fn recognize(
        &mut self,
        inotify_event: &inotify::Event,
    ) -> (Event, Option<i32>) {
        let wd = inotify_event.wd;

        match &inotify_event.kind {
            inotify::EventKind::Create(path, file_type) => {
                let full_path = self.full_path(wd, path);
                let event = match file_type {
                    inotify::FileType::Dir => {
                        Event::Create(full_path, FileType::Dir)
                    }
                    inotify::FileType::File => {
                        Event::Create(full_path, FileType::File)
                    }
                };
                (event, None)
            }

            inotify::EventKind::MoveFrom(from_path, file_type) => {
                let full_from_path = self.full_path(wd, from_path);
                if let Some(next_inotify_event) =
                    self.next_inotify_event().await
                {
                    match next_inotify_event.kind {
                        inotify::EventKind::MoveSelf => {
                            if next_inotify_event.wd != self.top_wd {
                                (
                                    Event::MoveAway(
                                        full_from_path,
                                        FileType::Dir,
                                    ),
                                    Some(next_inotify_event.wd),
                                )
                            } else {
                                self.cached_inotify_event =
                                    Some(next_inotify_event);
                                (
                                    Event::MoveAway(
                                        full_from_path,
                                        FileType::from(file_type),
                                    ),
                                    None,
                                )
                            }
                        }
                        inotify::EventKind::MoveTo(
                            ref to_path,
                            ref file_type,
                        ) => {
                            if inotify_event.cookie
                                != next_inotify_event.cookie
                            {
                                let file_type = FileType::from(file_type);
                                self.cached_inotify_event =
                                    Some(next_inotify_event);
                                (
                                    Event::MoveAway(full_from_path, file_type),
                                    None,
                                )
                            } else {
                                let full_to_path = self
                                    .full_path(next_inotify_event.wd, to_path);
                                if let Some(next2_inotify_event) =
                                    self.next_inotify_event().await
                                {
                                    match next2_inotify_event.kind {
                                        inotify::EventKind::MoveSelf => (
                                            Event::Move(
                                                full_from_path,
                                                full_to_path,
                                                FileType::Dir,
                                            ),
                                            Some(next2_inotify_event.wd),
                                        ),
                                        _ => {
                                            self.cached_inotify_event =
                                                Some(next2_inotify_event);
                                            (
                                                Event::Move(
                                                    full_from_path,
                                                    full_to_path,
                                                    FileType::from(file_type),
                                                ),
                                                None,
                                            )
                                        }
                                    }
                                } else {
                                    (
                                        Event::Move(
                                            full_from_path,
                                            full_to_path,
                                            FileType::from(file_type),
                                        ),
                                        None,
                                    )
                                }
                            }
                        }
                        _ => {
                            self.cached_inotify_event =
                                Some(next_inotify_event);
                            (
                                Event::MoveAway(
                                    full_from_path,
                                    FileType::from(file_type),
                                ),
                                None,
                            )
                        }
                    }
                } else {
                    (
                        Event::MoveAway(
                            full_from_path,
                            FileType::from(file_type),
                        ),
                        None,
                    )
                }
            }

            inotify::EventKind::MoveTo(path, file_type) => {
                let full_path = self.full_path(wd, path);
                let event = match file_type {
                    inotify::FileType::Dir => {
                        Event::MoveInto(full_path, FileType::Dir)
                    }
                    inotify::FileType::File => {
                        Event::MoveInto(full_path, FileType::File)
                    }
                };
                (event, None)
            }

            inotify::EventKind::Delete(path, file_type) => {
                let full_path = self.full_path(wd, path);
                if let Some(next_inotify_event) =
                    self.next_inotify_event().await
                {
                    match next_inotify_event.kind {
                        inotify::EventKind::DeleteSelf => {
                            if next_inotify_event.wd == self.top_wd {
                                self.cached_inotify_event =
                                    Some(next_inotify_event);
                                (
                                    Event::Delete(
                                        full_path,
                                        FileType::from(file_type),
                                    ),
                                    None,
                                )
                            } else {
                                (
                                    Event::Delete(full_path, FileType::Dir),
                                    Some(next_inotify_event.wd),
                                )
                            }
                        }
                        _ => {
                            self.cached_inotify_event =
                                Some(next_inotify_event);
                            (
                                Event::Delete(
                                    full_path,
                                    FileType::from(file_type),
                                ),
                                None,
                            )
                        }
                    }
                } else {
                    (Event::Delete(full_path, FileType::from(file_type)), None)
                }
            }

            inotify::EventKind::MoveSelf => {
                (Event::MoveTop(self.top_dir.to_owned()), None)
            }

            inotify::EventKind::DeleteSelf => {
                (Event::DeleteTop(self.top_dir.to_owned()), None)
            }

            inotify::EventKind::Modify(path) => {
                let full_path = self.full_path(wd, path);
                (Event::Modify(full_path, FileType::File), None)
            }
            inotify::EventKind::Access(path, file_type) => match path {
                Some(path) => {
                    let full_path = self.full_path(wd, path);
                    let event = match file_type {
                        inotify::FileType::Dir => {
                            Event::Access(full_path, FileType::Dir)
                        }
                        inotify::FileType::File => {
                            Event::Access(full_path, FileType::File)
                        }
                    };
                    (event, None)
                }
                None => {
                    if wd == self.top_wd {
                        (Event::AccessTop(self.top_dir.to_owned()), None)
                    } else {
                        (Event::Noise, None)
                    }
                }
            },
            inotify::EventKind::Attrib(path, file_type) => match path {
                Some(path) => {
                    let full_path = self.full_path(wd, path);
                    let event = match file_type {
                        inotify::FileType::Dir => {
                            Event::Attrib(full_path, FileType::Dir)
                        }
                        inotify::FileType::File => {
                            Event::Attrib(full_path, FileType::File)
                        }
                    };
                    (event, None)
                }
                None => {
                    if wd == self.top_wd {
                        (Event::AttribTop(self.top_dir.to_owned()), None)
                    } else {
                        (Event::Noise, None)
                    }
                }
            },
            inotify::EventKind::Open(path, file_type) => match path {
                Some(path) => {
                    let full_path = self.full_path(wd, path);
                    let event = match file_type {
                        inotify::FileType::Dir => {
                            Event::Open(full_path, FileType::Dir)
                        }
                        inotify::FileType::File => {
                            Event::Open(full_path, FileType::File)
                        }
                    };
                    (event, None)
                }
                None => {
                    if wd == self.top_wd {
                        (Event::OpenTop(self.top_dir.to_owned()), None)
                    } else {
                        (Event::Noise, None)
                    }
                }
            },
            inotify::EventKind::Close(path, file_type) => match path {
                Some(path) => {
                    let full_path = self.full_path(wd, path);
                    let event = match file_type {
                        inotify::FileType::Dir => {
                            Event::Close(full_path, FileType::Dir)
                        }
                        inotify::FileType::File => {
                            Event::Close(full_path, FileType::File)
                        }
                    };
                    (event, None)
                }
                None => {
                    if wd == self.top_wd {
                        (Event::CloseTop(self.top_dir.to_owned()), None)
                    } else {
                        (Event::Noise, None)
                    }
                }
            },

            inotify::EventKind::Unmount => {
                if inotify_event.wd == self.top_wd {
                    (Event::UnmountTop(self.top_dir.to_owned()), None)
                } else {
                    let full_path = self.path(wd);
                    (Event::Unmount(full_path, FileType::Dir), None)
                }
            }

            inotify::EventKind::Ignored => (Event::Ignored, None),
            inotify::EventKind::Unknown => (Event::Unknown, None),
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

fn guard(opts: WatcherOpts, path: &Path, file_type: FileType) -> bool {
    if file_type != FileType::Dir {
        return false;
    }
    if path.file_name().unwrap().as_bytes()[0] == b'.' {
        matches!(opts.sub_dotdir, Dotdir::Include)
    } else {
        true
    }
}

#[derive(PartialEq, Debug)]
pub enum FileType {
    Dir,
    File,
}

impl FileType {
    fn from(v: &inotify::FileType) -> Self {
        match v {
            inotify::FileType::Dir => Self::Dir,
            inotify::FileType::File => Self::File,
        }
    }
}

impl From<fs::FileType> for FileType {
    fn from(v: std::fs::FileType) -> Self {
        if v.is_dir() {
            Self::Dir
        } else {
            Self::File
        }
    }
}
