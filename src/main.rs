extern crate libc;

mod inotify;

use std::{env, fs::metadata, iter::Iterator, process::exit};

fn get_dirs(paths: impl Iterator<Item = String>) -> (Vec<String>, Vec<String>) {
    let mut dirs: Vec<String> = Vec::new();
    let mut others: Vec<String> = Vec::new();
    for p in paths {
        match metadata(&p) {
            Ok(path) => {
                if path.is_dir() {
                    dirs.push(p);
                } else {
                    others.push(p);
                }
            }
            Err(_) => {
                others.push(p);
            }
        }
    }
    (dirs, others)
}

fn main() {
    let (dirs, invalid_paths) = get_dirs(env::args().skip(1));
    if dirs.len() == 0 {
        eprintln!("invalid arguments: found no dirs!");
        exit(1);
    }
    if invalid_paths.len() > 0 {
        eprintln!("ignore path: {:?}", invalid_paths);
    }
    let mut watcher = inotify::build_watcher(&dirs);
    loop {
        let inotify_events = watcher.read_event();
        for e in inotify_events {
            println!("{}", e.path.display());
        }
    }
}
