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

fn main() {
    let opts: Opts = Opts::parse();

    let (dirs, invalid_paths): (HashSet<_>, HashSet<_>) = opts.paths.into_iter().partition(is_dir);
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

fn is_dir(path: &String) -> bool {
    if let Ok(p) = metadata(path) {
        p.is_dir()
    } else {
        false
    }
}
