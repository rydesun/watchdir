use std::{
    fs,
    ops::Deref,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::{ArgEnum, Clap, ValueHint};
use snafu::{ResultExt, Snafu};

pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " ",
    env!("BUILD_DATE"),
    " ",
    env!("GIT_SHA")
);

#[derive(Clap)]
#[clap(version = VERSION)]
#[clap(setting = clap::AppSettings::ColoredHelp)]
pub struct Opts {
    /// Include hidden subdirectories
    #[clap(name = "hidden", short = 'H', long)]
    pub include_hidden: bool,

    /// The directory to be watched
    #[clap(name = "DIR", value_hint = ValueHint::DirPath)]
    pub dir: Dir,

    /// Show debug messages
    #[clap(long)]
    pub debug: bool,

    /// Include modification events
    #[clap(long)]
    pub modify_event: bool,

    /// Canonicalize paths
    #[clap(long)]
    canonicalize: bool,

    /// When to use colors
    #[clap(value_name = "WHEN", long, arg_enum, default_value = "auto")]
    pub color: ColorWhen,
}

#[derive(ArgEnum)]
pub enum ColorWhen {
    Auto,
    Always,
    Ansi,
    Never,
}

pub struct Dir(PathBuf);

impl Deref for Dir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.0.as_path()
    }
}

impl FromStr for Dir {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let path = PathBuf::from(s);
        let metadata = fs::metadata(&path).context(InvalidPath {})?;
        if !metadata.is_dir() {
            Err(Error::NotDir)
        } else if fs::File::open(&path).is_err() {
            Err(Error::PermRead)
        } else {
            Ok(Self(path.join("")))
        }
    }
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("{}", source))]
    InvalidPath { source: std::io::Error },

    #[snafu(display("Not a valid directory path"))]
    NotDir,

    #[snafu(display("Permission denied"))]
    PermRead,

    #[snafu(display("Valid values are auto', 'always', 'ansi' or 'never'"))]
    OptionColor,
}

type Result<T, E = Error> = std::result::Result<T, E>;

pub fn parse() -> Opts {
    let mut opts = Opts::parse();
    if opts.canonicalize {
        opts.dir = Dir(opts.dir.canonicalize().unwrap().join(""));
    }
    opts
}
