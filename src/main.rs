extern crate libc;

mod inotify;

use std::{env, fs::metadata, iter::Iterator, process::exit};

fn get_dirs(paths: impl Iterator<Item = String>) -> (Vec<String>, Vec<String>) {
    let mut dirs: Vec<String> = Vec::new();
    let mut others: Vec<String> = Vec::new();
    for p in paths {
        if metadata(&p).unwrap().is_dir() {
            dirs.push(p);
        } else {
            others.push(p);
        }
    }
    (dirs, others)
}

fn main() {
    let (dirs, invalid_paths) = get_dirs(env::args().skip(1));
    if dirs.len() == 0 {
        eprint!("invalid arguments: found no dirs!\n");
        exit(1);
    }
    if invalid_paths.len() > 0 {
        eprintln!("ignore path: {:?}", invalid_paths);
    }
    let mut watcher = inotify::build_watcher(&dirs);
    loop {
        let inotify_event = watcher.read_event();
        println!("{}", inotify_event.path.display());
    }
}
