use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    fs::File,
    io::Read,
    iter::Iterator,
    mem::transmute,
    os::unix::io::FromRawFd,
    path::{Path, PathBuf},
};
use walkdir::{DirEntry, WalkDir};

const MAX_FILE_NAME: usize = 255;
const MAX_INOTIFY_EVENT_SIZE: usize = 16 + MAX_FILE_NAME + 1;

pub struct Watcher {
    f: File,
    fd: i32,
    wds: HashMap<i32, String>,
    cookies: HashMap<u32, String>,
    hidden: bool,
}

impl Drop for Watcher {
    fn drop(&mut self) {
        for wd in self.wds.keys() {
            unsafe { libc::inotify_rm_watch(self.fd, *wd) };
        }
    }
}

impl Watcher {
    pub fn new(dirs: &HashSet<String>, hidden: bool) -> Self {
        let fd = unsafe { libc::inotify_init() };
        let f = unsafe { File::from_raw_fd(fd) };

        let wds: HashMap<i32, String> = HashMap::new();
        let cookies: HashMap<u32, String> = HashMap::new();
        let mut watcher = Self {
            f,
            fd,
            wds,
            cookies,
            hidden,
        };
        for d in dirs.iter() {
            watcher.recursive_add_path(d, true);
        }
        watcher
    }

    pub fn read_event(&mut self) -> Vec<Event> {
        let mut buffer = [0; MAX_INOTIFY_EVENT_SIZE];
        let total = self.f.read(&mut buffer).expect("buffer overflow");
        let mut events = Vec::new();

        let mut p = 0;
        while p < total {
            let raw = &buffer[p..];
            let raw_event = self.get_raw_event(raw);
            if raw_event.mask & libc::IN_MOVED_FROM > 0 {
                let full_path = self.get_full_path(raw_event.wd, raw_event.path.clone());
                self.cookies
                    .insert(raw_event.cookie, full_path.to_string_lossy().to_string());
            }
            if raw_event.mask & libc::IN_MOVED_TO > 0 {
                let old_path = self.cookies.get(&raw_event.cookie).unwrap().clone();
                let new_path = self.get_full_path(raw_event.wd, raw_event.path.clone());
                let mut is_watched = false;
                for val in self.wds.values_mut() {
                    if *val == old_path {
                        *val = new_path.to_string_lossy().to_string();
                        is_watched = true;
                    }
                }
                if !is_watched && new_path.is_dir() {
                    events.push(Event {
                        path: new_path.clone(),
                    });
                    self.recursive_add_path(&new_path.to_string_lossy().to_string(), false);
                }
                self.cookies.remove(&raw_event.cookie);
            }
            if raw_event.mask & libc::IN_CREATE == 0 {
                p += 16 + raw_event.len as usize;
                continue;
            }
            let full_path = self.get_full_path(raw_event.wd, raw_event.path);
            events.push(Event {
                path: full_path.clone(),
            });

            if full_path.is_dir() {
                // Add new directory
                let new_deeper_dirs =
                    self.recursive_add_path(&full_path.to_string_lossy().to_string(), false);
                for i in new_deeper_dirs {
                    events.push(Event { path: i })
                }
            }
            p += 16 + raw_event.len as usize;
        }
        events
    }

    fn recursive_add_path(&mut self, d: &String, at_top: bool) -> Vec<PathBuf> {
        let mut new_dirs = Vec::new();
        let walker: Box<dyn Iterator<Item = Result<DirEntry, walkdir::Error>>>;
        walker = if self.hidden {
            Box::new(WalkDir::new(d).into_iter())
        } else {
            Box::new(
                WalkDir::new(d)
                    .into_iter()
                    .filter_entry(|e| (at_top && e.depth() == 0) || !is_hidden(e)),
            )
        };
        for entry in walker.filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            self.add_path(&path.to_string_lossy().to_string());
            if entry.depth() > 0 {
                new_dirs.push(PathBuf::from(path));
            }
        }
        new_dirs
    }

    fn add_path(&mut self, path: &String) {
        let ffi_path = CString::new(path.clone()).unwrap();
        let wd = unsafe {
            let event_types = libc::IN_CREATE | libc::IN_MOVE;
            libc::inotify_add_watch(self.fd, ffi_path.as_ptr() as *const i8, event_types)
        };
        self.wds.insert(wd, path.clone());
        eprintln!("Add new watch: {}", path);
    }

    fn get_raw_event(&self, raw: &[u8]) -> RawEvent {
        let mut raw_array = [0; 16];
        raw_array.copy_from_slice(&raw[..16]);
        let raw_event = unsafe { transmute::<[u8; 16], libc::inotify_event>(raw_array) };
        let raw_path = String::from_utf8(raw[16..(16 + raw_event.len as usize)].to_vec())
            .expect("invalid text");
        let path = raw_path.trim_matches(char::from(0)).to_string();
        RawEvent {
            wd: raw_event.wd,
            mask: raw_event.mask,
            cookie: raw_event.cookie,
            len: raw_event.len,
            path,
        }
    }

    fn get_full_path(&self, wd: i32, path: String) -> PathBuf {
        let dir = self.wds[&wd].clone();
        Path::new(&dir).join(path)
    }
}

#[derive(Debug)]
struct RawEvent {
    wd: i32,
    mask: u32,
    cookie: u32,
    len: u32,
    path: String,
}

pub struct Event {
    pub path: PathBuf,
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}
