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
    wds: HashMap<i32, String>,
}

pub fn build_watcher(paths: &Vec<String>) -> Watcher {
    let fd = unsafe { libc::inotify_init() };
    let f = unsafe { File::from_raw_fd(fd) };

    let mut wds: HashMap<i32, String> = HashMap::new();
    for p in paths.iter() {
        let path = CString::new(p.clone()).unwrap();
        let wd =
            unsafe { libc::inotify_add_watch(fd, path.as_ptr() as *const i8, libc::IN_CREATE) };
        wds.insert(wd, p.to_string());
    }
    Watcher { f, wds }
}

impl Watcher {
    pub fn read_event(&mut self) -> Event {
        let mut buffer = [0; MAX_INOTIFY_EVENT_SIZE];
        self.f.read(&mut buffer).expect("buffer overflow");
        let mut raw_wd = [0; 4];
        raw_wd.copy_from_slice(&buffer[..4]);
        let wd = unsafe { transmute::<[u8; 4], i32>(raw_wd) };
        let raw_path = String::from_utf8(buffer[16..].to_vec()).expect("invalid text");
        let path = raw_path.trim_matches(char::from(0)).to_string();
        let full_path = self.get_full_path(wd, path);
        Event { path: full_path }
    }
    fn get_full_path(&self, wd: i32, path: String) -> PathBuf {
        let dir = self.wds[&wd].clone();
        Path::new(&dir).join(path)
    }
}

pub struct Event {
    pub path: PathBuf,
}
