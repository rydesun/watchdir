mod cli;
mod inotify;
mod path_tree;
mod watcher;

use std::{io::Write, ops::Deref, path::Path};

use clap::Clap;
use mimalloc::MiMalloc;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use tracing::{error, info, warn, Level};
use tracing_subscriber::EnvFilter;
use watcher::Event;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    let opts = cli::Opts::parse();

    let mut color_spec = ColorSpec::new();
    let mut stdout = StandardStream::stdout((&opts.color).into());

    init_logger(opts.debug, match opts.color {
        cli::ColorWhen::Always => true,
        cli::ColorWhen::Ansi => true,
        cli::ColorWhen::Auto => isatty_stderr(),
        cli::ColorWhen::Never => false,
    });
    info!("version: {}", cli::VERSION);

    let watcher = match watcher::Watcher::new(
        opts.dir.deref(),
        watcher::WatcherOpts::new(
            opts.include_hidden.into(),
            opts.modify_event,
        ),
    ) {
        Ok(watcher) => watcher,
        Err(e) => {
            error!("{}", e);
            std::process::exit(1);
        }
    };
    info!("initialized successfully and listening to upcoming events...\n");

    for event in watcher {
        print_event(&mut stdout, &mut color_spec, &event, &opts.dir).unwrap();
        match event {
            watcher::Event::MoveTop(_) => {
                warn!(
                    "Watched dir was moved. The prefix of path can no longer \
                     be trusted!"
                );
            }
            watcher::Event::DeleteTop(_) => {
                warn!("Watched dir was deleted.");
                std::process::exit(0);
            }
            _ => {}
        }
    }
}

fn print_event(
    stdout: &mut StandardStream,
    color_spec: &mut ColorSpec,
    event: &watcher::Event,
    _path_prefix: &Path,
) -> Result<(), std::io::Error> {
    let (head, path, color) = match event {
        Event::Create(path) => ("Create", Some(path), Color::Green),
        Event::Delete(path) => ("Delete", Some(path), Color::Magenta),
        Event::Move(..) => ("Move", None, Color::Blue),
        Event::MoveAway(path) => ("MoveAway", Some(path), Color::Blue),
        Event::MoveInto(path) => ("MoveInto", Some(path), Color::Blue),
        Event::Modify(path) => ("Modify", Some(path), Color::Yellow),
        Event::MoveTop(path) => ("MoveTop", Some(path), Color::Red),
        Event::DeleteTop(path) => ("DeleteTop", Some(path), Color::Red),
        Event::Unknown => ("Unknown", None, Color::Red),
        Event::Ignored => return Ok(()),
    };

    stdout.set_color(color_spec.set_fg(Some(color)))?;
    write!(stdout, "{:<12}", head)?;

    if let Some(path) = path {
        writeln!(stdout, "{:?}", path)?;
    } else if let Event::Move(from, to) = event {
        writeln!(stdout, "{:?} -> {:?}", from, to)?;
    }

    Ok(())
}

fn init_logger(debug: bool, color: bool) {
    let subscriber = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(color);
    if debug {
        subscriber
            .with_env_filter(EnvFilter::new(Level::DEBUG.to_string()))
            .init();
    } else {
        subscriber.init();
    };
}

fn isatty_stdout() -> bool {
    unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 }
}

fn isatty_stderr() -> bool {
    unsafe { libc::isatty(libc::STDERR_FILENO) != 0 }
}

impl From<&cli::ColorWhen> for ColorChoice {
    fn from(v: &cli::ColorWhen) -> Self {
        match v {
            cli::ColorWhen::Always => Self::Always,
            cli::ColorWhen::Ansi => Self::AlwaysAnsi,
            cli::ColorWhen::Auto => {
                if isatty_stdout() {
                    Self::Auto
                } else {
                    Self::Never
                }
            }
            cli::ColorWhen::Never => ColorChoice::Never,
        }
    }
}
