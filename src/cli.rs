use clap::{Clap, ValueHint};
use std::{fs::metadata, iter::Iterator, path::PathBuf, process::exit};

#[derive(Clap)]
#[clap(setting = clap::AppSettings::ColoredHelp)]
pub struct Opts {
    /// Include hidden files and directories
    #[clap(short = 'H', long)]
    pub hidden: bool,

    /// Directories watched
    #[clap(name = "DIR", parse(from_os_str), value_hint = ValueHint::AnyPath)]
    pub dirs: Vec<PathBuf>,
}

pub fn parse() -> Opts {
    let mut opts = Opts::parse();

    let dirs: Vec<PathBuf> = opts
        .dirs
        .into_iter()
        .filter(|p| {
            if let Ok(pm) = metadata(p) {
                if pm.is_dir() {
                    true
                } else {
                    eprintln!("Skip non-directory path: {}", p.display());
                    false
                }
            } else {
                eprintln!("Skip invalid path: {}", p.display());
                false
            }
        })
        .collect();
    if dirs.len() == 0 {
        eprintln!("Must contain at least one valid path!");
        exit(1);
    }

    opts.dirs = dirs;
    opts
}
