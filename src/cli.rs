use std::{fs, path::PathBuf, str::FromStr};

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
    /// Include hidden files and directories
    #[clap(short = 'H', long)]
    pub hidden: bool,

    /// Directory to watch
    #[clap(name = "DIR", parse(from_os_str), value_hint = ValueHint::DirPath)]
    pub dir: PathBuf,

    /// A level of verbosity, and can be used up to 2 times
    #[clap(short, long, parse(from_occurrences))]
    pub verbose: i32,

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

pub fn parse() -> Result<Opts> {
    let opts = Opts::parse();

    let metadata = fs::metadata(&opts.dir).context(InvalidPath {})?;
    if !metadata.is_dir() {
        Err(Error::NotDir)
    } else if fs::File::open(&opts.dir).is_err() {
        Err(Error::PermRead)
    } else {
        Ok(opts)
    }
}
