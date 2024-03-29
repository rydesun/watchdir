use std::{
    fs,
    ops::Deref,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::{IntoApp, Parser, ValueHint};
use clap_complete::{generate, shells};
use clap_derive::{ArgEnum, Parser};
use lazy_static::lazy_static;
use snafu::{ResultExt, Snafu};

lazy_static! {
    pub static ref VERSION: String =
        [env!("CARGO_PKG_VERSION"), env!("BUILD_DATE"), &env!("GIT_SHA")[..5]]
            .join(" ");
}

#[derive(Parser)]
#[clap(version = VERSION.as_str())]
#[clap(color = clap::ColorChoice::Auto)]
#[clap(term_width = 79)]
pub struct Opts {
    /// Include hidden subdirectories
    #[clap(short = 'H', long)]
    pub include_hidden: bool,

    /// The directory to be watched
    #[clap(name = "DIR", value_hint = ValueHint::DirPath,
        required_unless_present_any = ["completion"])]
    pub dir: Option<Dir>,

    /// Show debug messages
    #[clap(long)]
    pub debug: bool,

    /// Include extra events
    #[clap(value_name = "EVENT_TYPE", long, arg_enum, use_delimiter = true)]
    pub extra_events: Vec<ExtraEvent>,

    /// Exclude default events
    #[clap(value_name = "EVENT_TYPE", long, arg_enum, use_delimiter = true)]
    pub exclude_events: Vec<Event>,

    /// Canonicalize paths
    #[clap(long)]
    canonicalize: bool,

    /// List events per line
    #[clap(long)]
    pub oneline: bool,

    /// Strip watched directory path
    #[clap(long = "no-prefix", parse(from_flag = std::ops::Not::not))]
    pub prefix: bool,

    /// Print time
    #[clap(short, long)]
    pub time: bool,

    /// When to use colors
    #[clap(value_name = "WHEN", long, arg_enum, default_value = "auto")]
    pub color: ColorWhen,

    /// Generate completions for shell
    #[clap(value_name = "SHELL", long, arg_enum)]
    pub completion: Option<Shell>,

    /// Throttle modify event for some milliseconds
    #[clap(value_name = "TIME", long, default_value = "1000")]
    pub throttle_modify: u64,
}

#[derive(ArgEnum, Clone)]
pub enum Event {
    Create,
    Delete,
    Move,
    Unmount,
}

#[derive(ArgEnum, Clone)]
pub enum ExtraEvent {
    Modify,
    Access,
    Attrib,
    Open,
    Close,
}

#[derive(ArgEnum, Clone)]
pub enum ColorWhen {
    Auto,
    Always,
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

#[derive(Parser, ArgEnum, Clone, PartialEq)]
pub enum Shell {
    Bash,
    Fish,
    Zsh,
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

    if let Some(shell) = opts.completion {
        print_completions(shell);
        std::process::exit(0);
    }

    if opts.canonicalize {
        opts.dir =
            Some(Dir(opts.dir.unwrap().canonicalize().unwrap().join("")));
    }
    opts
}

pub fn print_completions(shell: Shell) {
    let mut buf = std::io::stdout();
    let mut app = Opts::into_app();
    let name = app.get_name().to_string();
    match shell {
        Shell::Bash => generate(shells::Bash, &mut app, name, &mut buf),
        Shell::Fish => generate(shells::Fish, &mut app, name, &mut buf),
        Shell::Zsh => generate(shells::Zsh, &mut app, name, &mut buf),
    }
}
