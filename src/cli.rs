use std::{fs::metadata, iter::Iterator, path::PathBuf, process::exit};

use clap::{Clap, ValueHint};
use tracing::{error, warn};

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " ", env!("GIT_SHA"));

#[derive(Clap)]
#[clap(version = VERSION)]
#[clap(setting = clap::AppSettings::ColoredHelp)]
pub struct Opts {
    /// Include hidden files and directories
    #[clap(short = 'H', long)]
    pub hidden: bool,

    /// Directories watched
    #[clap(name = "DIR", parse(from_os_str), value_hint = ValueHint::AnyPath)]
    pub dirs: Vec<PathBuf>,

    /// A level of verbosity, and can be used up to 2 times
    #[clap(short, long, parse(from_occurrences))]
    pub verbose: i32,
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
                    warn!("Skip non-directory path: {}", p.display());
                    false
                }
            } else {
                warn!("Skip invalid path: {}", p.display());
                false
            }
        })
        .collect();
    if dirs.is_empty() {
        error!("Must contain at least one valid path!");
        exit(1);
    }

    opts.dirs = dirs;
    opts
}
