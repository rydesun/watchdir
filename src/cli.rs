use std::{
    fs,
    ops::Deref,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::{Clap, ValueHint};
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

    /// Directory to watch
    #[clap(name = "DIR", value_hint = ValueHint::DirPath)]
    pub dir: Dir,

    /// Show debug messages
    #[clap(long)]
    pub debug: bool,

    /// Also includes modification events
    #[clap(long)]
    pub modify_event: bool,

    /// Canonicalize paths
    #[clap(long)]
    canonicalize: bool,

    /// When to use colors. WHEN can be 'auto', 'always', 'ansi', or 'never'
    #[clap(value_name = "WHEN", long, default_value = "auto")]
    pub color: ColorWhen,
}

pub enum ColorWhen {
    Auto,
    Always,
    Ansi,
    Never,
}

impl FromStr for ColorWhen {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "ansi" => Ok(Self::Ansi),
            "never" => Ok(Self::Never),
            _ => Err(Error::OptionColor),
        }
    }
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
