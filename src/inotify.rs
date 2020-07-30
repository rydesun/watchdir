use std::{
    collections::HashMap,
    ffi::CString,
    fs::File,
    io::Read,
    mem::transmute,
    os::unix::io::FromRawFd,
    path::{Path, PathBuf},
};

const MAX_FILE_NAME: usize = 255;
const MAX_INOTIFY_EVENT_SIZE: usize = 16 + MAX_FILE_NAME + 1;

pub struct Watcher {
    f: File,
    fd: i32,
    wds: HashMap<i32, String>,
}

impl Drop for Watcher {
    fn drop(&mut self) {
        for wd in self.wds.keys() {
            unsafe { libc::inotify_rm_watch(self.fd, *wd) };
        }
    }
}

impl Watcher {
    pub fn new(paths: &Vec<String>) -> Self {
        let fd = unsafe { libc::inotify_init() };
        let f = unsafe { File::from_raw_fd(fd) };

        let mut wds: HashMap<i32, String> = HashMap::new();
        for p in paths.iter() {
            let path = CString::new(p.clone()).unwrap();
            let wd =
                unsafe { libc::inotify_add_watch(fd, path.as_ptr() as *const i8, libc::IN_CREATE) };
            wds.insert(wd, p.to_string());
        }
        Watcher { f, fd, wds }
    }
    pub fn read_event(&mut self) -> Vec<Event> {
        let mut buffer = [0; MAX_INOTIFY_EVENT_SIZE];
        let total = self.f.read(&mut buffer).expect("buffer overflow");
        let mut events: Vec<Event> = Vec::new();

        let mut p = 0;
        while p < total {
            let raw = &buffer[p..];
            let raw_event = self.get_raw_event(raw);
            let full_path = self.get_full_path(raw_event.wd, raw_event.path);
            events.push(Event { path: full_path });

            p += 16 + raw_event.len as usize;
        }
        events
    }
    fn get_raw_event(&self, raw: &[u8]) -> RawEvent {
        let mut raw_wd = [0; 4];
        let mut raw_len = [0; 4];
        raw_wd.copy_from_slice(&raw[..4]);
        raw_len.copy_from_slice(&raw[12..16]);
        let wd = unsafe { transmute::<[u8; 4], i32>(raw_wd) };
        let len = unsafe { transmute::<[u8; 4], u32>(raw_len) };
        let raw_path =
            String::from_utf8(raw[16..(16 + len as usize)].to_vec()).expect("invalid text");
        let path = raw_path.trim_matches(char::from(0)).to_string();
        RawEvent { wd, len, path }
    }
    fn get_full_path(&self, wd: i32, path: String) -> PathBuf {
        let dir = self.wds[&wd].clone();
        Path::new(&dir).join(path)
    }
}

struct RawEvent {
    wd: i32,
    len: u32,
    path: String,
}

pub struct Event {
    pub path: PathBuf,
}
