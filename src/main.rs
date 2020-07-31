extern crate libc;

mod inotify;

use clap::Clap;
use std::{collections::HashSet, fs::metadata, iter::Iterator, process::exit};

#[derive(Clap)]
#[clap(author = "rydesun <rydesun@gmail.com>")]
struct Opts {
    #[clap(long)]
    hidden: bool,
    paths: Vec<String>,
}

fn get_dirs(paths: Vec<String>) -> (HashSet<String>, HashSet<String>) {
    let mut dirs: HashSet<String> = HashSet::new();
    let mut others: HashSet<String> = HashSet::new();
    for p in paths {
        match metadata(&p) {
            Ok(path) => {
                if path.is_dir() {
                    dirs.insert(p);
                } else {
                    others.insert(p);
                }
            }
            Err(_) => {
                others.insert(p);
            }
        }
    }
    (dirs, others)
}

fn main() {
    let opts: Opts = Opts::parse();

    let (dirs, invalid_paths) = get_dirs(opts.paths);
    if dirs.len() == 0 {
        eprintln!("invalid arguments: found no dirs!");
        exit(1);
    }
    if invalid_paths.len() > 0 {
        eprintln!("ignore path: {:?}", invalid_paths);
    }
    let mut watcher = inotify::Watcher::new(&dirs, opts.hidden);
    loop {
        let inotify_events = watcher.read_event();
        for e in inotify_events {
            println!("{}", e.path.display());
        }
    }
}
