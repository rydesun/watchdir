use std::{
    fs,
    ops::Deref,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::{ArgEnum, Clap, IntoApp, ValueHint};
use clap_generate::{generate, generators, Generator};
use lazy_static::lazy_static;
use snafu::{ResultExt, Snafu};

lazy_static! {
    pub static ref VERSION: String =
        [env!("CARGO_PKG_VERSION"), env!("BUILD_DATE"), &env!("GIT_SHA")[..5]]
            .join(" ");
}

#[derive(Clap)]
#[clap(version = VERSION.as_str())]
#[clap(setting = clap::AppSettings::ColoredHelp)]
#[clap(term_width = 79)]
pub struct Opts {
    /// Include hidden subdirectories
    #[clap(name = "hidden", short = 'H', long)]
    pub include_hidden: bool,

    /// The directory to be watched
    #[clap(name = "DIR", value_hint = ValueHint::DirPath,
        required_unless_present_any = ["completion"])]
    pub dir: Option<Dir>,

    /// Show debug messages
    #[clap(long)]
    pub debug: bool,

    /// Include modification events
    #[clap(long)]
    pub modify_event: bool,

    /// Canonicalize paths
    #[clap(long)]
    canonicalize: bool,

    /// Strip watched directory path
    #[clap(long = "no-prefix", parse(from_flag = std::ops::Not::not))]
    pub prefix: bool,

    /// When to use colors
    #[clap(value_name = "WHEN", long, arg_enum, default_value = "auto")]
    pub color: ColorWhen,

    /// Generate completions for shell
    #[clap(value_name = "SHELL", long, arg_enum)]
    pub completion: Option<Shell>,
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

#[derive(ArgEnum, Clap, PartialEq)]
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
    if opts.canonicalize {
        opts.dir =
            Some(Dir(opts.dir.unwrap().canonicalize().unwrap().join("")));
    }
    opts
}

pub fn print_completions(shell: Shell) {
    fn print<G: Generator>() {
        let mut buf = std::io::stdout();
        let mut app = Opts::into_app();
        let name = app.get_name().to_string();
        generate::<G, _>(&mut app, name, &mut buf);
    }
    match shell {
        Shell::Bash => print::<generators::Bash>(),
        Shell::Fish => print::<generators::Fish>(),
        Shell::Zsh => print::<generators::Zsh>(),
    }
}
