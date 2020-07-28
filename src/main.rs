extern crate libc;

use std::{
    collections::HashMap,
    env,
    ffi::CString,
    fs::{metadata, File},
    io::Read,
    mem::transmute,
    os::unix::io::FromRawFd,
    path::Path,
    process::exit,
};

const MAX_FILE_NAME: usize = 255;
const MAX_INOTIFY_EVENT_SIZE: usize = 16 + MAX_FILE_NAME + 1;

fn main() {
    let mut dirs: Vec<String> = Vec::new();
    let mut invalid_paths: Vec<String> = Vec::new();
    for p in env::args().skip(1) {
        if metadata(&p).unwrap().is_dir() {
            dirs.push(p);
        } else {
            invalid_paths.push(p);
        }
    }
    if dirs.len() == 0 {
        eprint!("invalid arguments: found no dirs!\n");
        exit(1);
    }
    if invalid_paths.len() > 0 {
        eprintln!("ignore path: {:?}", invalid_paths);
    }

    let fd = unsafe { libc::inotify_init() };
    let mut f = unsafe { File::from_raw_fd(fd) };

    let mut wds = HashMap::new();
    for d in dirs {
        let path = CString::new(d.clone()).unwrap();
        let wd =
            unsafe { libc::inotify_add_watch(fd, path.as_ptr() as *const i8, libc::IN_CREATE) };
        wds.insert(wd, d);
    }

    loop {
        let mut buffer = [0; MAX_INOTIFY_EVENT_SIZE];
        f.read(&mut buffer).expect("buffer overflow");
        let mut raw_wd = [0; 4];
        raw_wd.copy_from_slice(&buffer[..4]);
        let wd = unsafe { transmute::<[u8; 4], i32>(raw_wd) };
        let raw_path = String::from_utf8(buffer[16..].to_vec()).expect("invalid text");
        let path = raw_path.trim_matches(char::from(0));
        println!("{}", Path::new(&wds[&wd]).join(path).display());
    }
}
