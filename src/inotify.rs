use std::{
    ffi::{CStr, OsStr},
    fs::File,
    io::Read,
    mem::size_of,
    os::unix::{ffi::OsStrExt, io::FromRawFd},
    path::PathBuf,
};

use tracing::{debug, instrument};

const MAX_FILENAME_LENGTH: usize = 255;
const INOTIFY_EVENT_HEADER_SIZE: usize = size_of::<libc::inotify_event>();
const MAX_INOTIFY_EVENT_SIZE: usize =
    INOTIFY_EVENT_HEADER_SIZE + MAX_FILENAME_LENGTH + 1;

pub struct EventSeq {
    file: File,
    buffer: [u8; MAX_INOTIFY_EVENT_SIZE],
    len: usize,
    offset: usize,
}

impl EventSeq {
    pub fn new(fd: i32) -> Self {
        Self {
            file: unsafe { File::from_raw_fd(fd) },
            buffer: [0; MAX_INOTIFY_EVENT_SIZE],
            len: 0,
            offset: 0,
        }
    }

    #[instrument(skip(self), fields(len=self.len, offset=self.offset))]
    fn parse(&self) -> Event {
        let raw = &self.buffer[self.offset..];
        let raw_event: libc::inotify_event =
            unsafe { std::ptr::read(raw.as_ptr() as *const _) };

        let kind = if raw_event.mask & libc::IN_MOVED_FROM > 0 {
            EventKind::MoveFrom
        } else if raw_event.mask & libc::IN_MOVED_TO > 0 {
            EventKind::MoveTo
        } else if raw_event.mask & libc::IN_CREATE > 0 {
            EventKind::Create
        } else if raw_event.mask & libc::IN_MOVE_SELF > 0 {
            EventKind::MoveSelf
        } else if raw_event.mask & libc::IN_IGNORED > 0 {
            EventKind::Ignored
        } else {
            EventKind::Unknown
        };
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

        let event = Event {
            wd: raw_event.wd,
            cookie: raw_event.cookie,
            len: raw_event.len,
            kind,
            path,
        };
        debug!(?event);

        event
    }
}

impl Iterator for EventSeq {
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.len {
            self.buffer.fill(0);
            self.offset = 0;
        }
        if self.offset == 0 {
            self.len = self.file.read(&mut self.buffer).unwrap();
        }

        let event = self.parse();
        self.offset += INOTIFY_EVENT_HEADER_SIZE + event.len as usize;
        Some(event)
    }
}

#[derive(Debug)]
pub struct Event {
    pub wd: i32,
    pub cookie: u32,
    len: u32,
    pub path: Option<PathBuf>,
    pub kind: EventKind,
}

#[derive(Debug)]
pub enum EventKind {
    MoveTo,
    MoveFrom,
    MoveSelf,
    Create,
    Ignored,
    Unknown,
}
