use std::{
    ffi::{CStr, OsStr},
    fs::File,
    io::Read,
    mem::size_of,
    os::unix::{ffi::OsStrExt, io::FromRawFd},
    path::PathBuf,
};

const MAX_FILENAME_LENGTH: usize = 255;
const INOTIFY_EVENT_HEADER_SIZE: usize = size_of::<libc::inotify_event>();
const MAX_INOTIFY_EVENT_SIZE: usize = INOTIFY_EVENT_HEADER_SIZE + MAX_FILENAME_LENGTH + 1;

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

    fn parse(&self) -> (RawEvent, Event) {
        let raw = &self.buffer[self.offset..];
        let raw_event: libc::inotify_event = unsafe { std::ptr::read(raw.as_ptr() as *const _) };

        let kind = if raw_event.mask & libc::IN_MOVED_FROM > 0 {
            EventKind::MoveFrom
        } else if raw_event.mask & libc::IN_MOVED_TO > 0 {
            EventKind::MoveTo
        } else if raw_event.mask & libc::IN_CREATE > 0 {
            EventKind::Create
        } else {
            EventKind::Unknown
        };
        let path = {
            let raw_path = unsafe {
                CStr::from_bytes_with_nul_unchecked(
                    raw[INOTIFY_EVENT_HEADER_SIZE
                        ..(INOTIFY_EVENT_HEADER_SIZE + raw_event.len as usize)]
                        .split_inclusive(|c| *c == 0)
                        .next()
                        .unwrap(),
                )
            };
            PathBuf::from(OsStr::from_bytes(raw_path.to_bytes()))
        };

        (
            RawEvent {
                wd: raw_event.wd,
                cookie: raw_event.cookie,
                len: raw_event.len,
            },
            Event { kind, path },
        )
    }
}

impl Iterator for EventSeq {
    type Item = (RawEvent, Event);
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.len {
            self.buffer.fill(0);
            self.offset = 0;
        }
        if self.offset == 0 {
            self.len = self.file.read(&mut self.buffer).unwrap();
        }

        let (raw_event, event) = self.parse();
        self.offset += INOTIFY_EVENT_HEADER_SIZE + raw_event.len as usize;
        Some((raw_event, event))
    }
}

#[derive(Debug)]
pub struct RawEvent {
    pub wd: i32,
    pub cookie: u32,
    len: u32,
}

#[derive(Debug)]
pub struct Event {
    pub path: PathBuf,
    pub kind: EventKind,
}

#[derive(Debug)]
pub enum EventKind {
    MoveTo,
    MoveFrom,
    Create,
    Unknown,
}
