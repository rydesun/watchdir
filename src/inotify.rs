use std::{
    ffi::{CStr, OsStr},
    mem::size_of,
    os::unix::{ffi::OsStrExt, io::FromRawFd},
    path::PathBuf,
};

use async_stream::stream;
use futures::Stream;
use tokio::{fs::File, io::AsyncReadExt};
use tracing::{debug, instrument};

const MAX_FILENAME_LENGTH: usize = 255;
const INOTIFY_EVENT_HEADER_SIZE: usize = size_of::<libc::inotify_event>();
const MAX_INOTIFY_EVENT_SIZE: usize =
    INOTIFY_EVENT_HEADER_SIZE + MAX_FILENAME_LENGTH + 1;

pub struct EventSeq {
    #[allow(dead_code)]
    fd: i32,
    file: File,
    pollfd: libc::pollfd,
    buffer: [u8; MAX_INOTIFY_EVENT_SIZE],
    len: usize,
    offset: usize,
}

impl EventSeq {
    pub fn new(fd: i32) -> Self {
        Self {
            fd,
            file: unsafe { File::from_raw_fd(fd) },
            pollfd: libc::pollfd { fd, events: libc::POLLIN, revents: 0 },
            buffer: [0; MAX_INOTIFY_EVENT_SIZE],
            len: 0,
            offset: 0,
        }
    }

    pub fn stream(&mut self) -> impl Stream<Item = Event> + '_ {
        stream! {
            loop {
                if self.offset >= self.len {
                    self.buffer.fill(0);
                    self.offset = 0;
                }
                if self.offset == 0 {
                        self.len = self.file.read(&mut self.buffer).await.unwrap();
                }

                let event = self.parse();
                self.offset += INOTIFY_EVENT_HEADER_SIZE + event.len as usize;
                yield event
            }
        }
    }

    #[instrument(skip(self), fields(len=self.len, offset=self.offset))]
    fn parse(&self) -> Event {
        let raw = &self.buffer[self.offset..];
        let raw_event: libc::inotify_event;
        loop {
            let res: libc::inotify_event =
                unsafe { std::ptr::read(raw.as_ptr() as *const _) };
            if res.wd > 0 {
                raw_event = res;
                break;
            } else {
                // FIXME: What happened?
                debug!("Invalid inotify event");
            }
        }

        let path = if raw_event.len > 0 {
            let raw_path = unsafe {
                CStr::from_bytes_with_nul_unchecked(
                    raw[INOTIFY_EVENT_HEADER_SIZE
                        ..(INOTIFY_EVENT_HEADER_SIZE
                            + raw_event.len as usize)]
                        .split_inclusive(|c| *c == 0)
                        .next()
                        .unwrap(),
                )
            };
            Some(PathBuf::from(OsStr::from_bytes(raw_path.to_bytes())))
        } else {
            None
        };

        let file_type = if raw_event.mask & libc::IN_ISDIR > 0 {
            FileType::Dir
        } else {
            FileType::File
        };

        let kind = match raw_event.mask {
            i if i & libc::IN_MOVED_FROM > 0 => {
                EventKind::MoveFrom(path.unwrap(), file_type)
            }
            i if i & libc::IN_MOVED_TO > 0 => {
                EventKind::MoveTo(path.unwrap(), file_type)
            }
            i if i & libc::IN_CREATE > 0 => {
                EventKind::Create(path.unwrap(), file_type)
            }
            i if i & libc::IN_MOVE_SELF > 0 => EventKind::MoveSelf,
            i if i & libc::IN_DELETE > 0 => {
                EventKind::Delete(path.unwrap(), file_type)
            }
            i if i & libc::IN_DELETE_SELF > 0 => EventKind::DeleteSelf,
            i if i & libc::IN_MODIFY > 0 => EventKind::Modify(path.unwrap()),
            i if i & libc::IN_ATTRIB > 0 => EventKind::Attrib(path, file_type),
            i if i & libc::IN_ACCESS > 0 => EventKind::Access(path, file_type),
            i if i & libc::IN_OPEN > 0 => EventKind::Open(path, file_type),
            i if i & libc::IN_CLOSE > 0 => EventKind::Close(path, file_type),
            i if i & libc::IN_UNMOUNT > 0 => EventKind::Unmount,
            i if i & libc::IN_IGNORED > 0 => EventKind::Ignored,
            _ => EventKind::Unknown,
        };

        let event = Event {
            wd: raw_event.wd,
            cookie: raw_event.cookie,
            len: raw_event.len,
            kind,
        };
        debug!(?event);

        event
    }

    pub fn has_next_event(&mut self) -> bool {
        // HACK: These milliseconds represent the waiting for next event.
        // Consider a more appropriate value.
        const TIMEOUT: i32 = 1;

        if self.offset >= self.len {
            // XXX: ioctl is invalid
            // let n = unsafe { libc::ioctl(self.fd, libc::FIONREAD) };
            let n = unsafe { libc::poll(&mut self.pollfd, 1, TIMEOUT) };
            debug!("Check if there are more events: n = {}", n);
            n > 0
        } else {
            debug!("{}", "Buffer has content");
            true
        }
    }
}

#[derive(Debug)]
pub struct Event {
    pub kind: EventKind,
    pub wd: i32,
    pub cookie: u32,
    len: u32,
}

#[derive(Debug)]
pub enum EventKind {
    MoveTo(PathBuf, FileType),
    MoveFrom(PathBuf, FileType),
    MoveSelf,
    Create(PathBuf, FileType),
    Delete(PathBuf, FileType),
    DeleteSelf,
    Modify(PathBuf),
    Access(Option<PathBuf>, FileType),
    Attrib(Option<PathBuf>, FileType),
    Open(Option<PathBuf>, FileType),
    Close(Option<PathBuf>, FileType),
    Unmount,
    Ignored,
    Unknown,
}

#[derive(Debug)]
pub enum FileType {
    Dir,
    File,
}
