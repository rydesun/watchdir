mod cli;
mod inotify;
mod path_tree;
mod watcher;

use std::{io::Write, path::Path};

use mimalloc::MiMalloc;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use tracing::{error, info, warn, Level};
use tracing_subscriber::EnvFilter;
use watcher::Event;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    let opts = cli::parse();
    if let Some(shell) = opts.completion {
        cli::print_completions(shell);
        std::process::exit(0);
    }

    let mut stdout = StandardStream::stdout((&opts.color).into());

    init_logger(opts.debug, match opts.color {
        cli::ColorWhen::Always => true,
        cli::ColorWhen::Ansi => true,
        cli::ColorWhen::Auto => isatty_stderr(),
        cli::ColorWhen::Never => false,
    });
    info!("version: {}", cli::VERSION);

    let watcher = match watcher::Watcher::new(
        opts.dir.as_ref().unwrap(),
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
        print_event(&mut stdout, &event, opts.dir.as_ref().unwrap()).unwrap();
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
    event: &watcher::Event,
    path_prefix: &Path,
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

    stdout.set_color(ColorSpec::new().set_fg(Some(color)).set_bold(true))?;
    write!(stdout, "{:<12}", head)?;

    match event {
        Event::Move(from, to) => {
            let from_rest = from.strip_prefix(path_prefix).unwrap();
            let _from_rest_parent =
                from_rest.parent().unwrap_or_else(|| Path::new("")).join("");
            let _from_rest_name = from_rest.file_name().unwrap();
            let to_rest = to.strip_prefix(path_prefix).unwrap();
            let _to_rest_parent =
                to_rest.parent().unwrap_or_else(|| Path::new("")).join("");
            let _to_rest_name = to_rest.file_name().unwrap();

            stdout.set_color(ColorSpec::new().set_dimmed(true))?;
            write!(stdout, "{}", path_prefix.to_string_lossy())?;

            stdout.set_color(
                ColorSpec::new().set_fg(Some(color)).set_bold(true),
            )?;
            write!(stdout, "{}", from_rest.to_string_lossy())?;

            stdout.set_color(ColorSpec::new().set_dimmed(true))?;
            write!(stdout, " -> ")?;

            stdout.set_color(ColorSpec::new().set_dimmed(true))?;
            write!(stdout, "{}", path_prefix.to_string_lossy())?;

            stdout.set_color(
                ColorSpec::new().set_fg(Some(color)).set_bold(true),
            )?;
            write!(stdout, "{}", to_rest.to_string_lossy())?;
        }
        Event::MoveTop(path) | Event::DeleteTop(path) => {
            stdout.set_color(
                ColorSpec::new().set_fg(Some(color)).set_bold(true),
            )?;
            write!(stdout, "{}", path.to_string_lossy())?;
        }
        _ => {
            let path = path.unwrap();
            let path_rest = path.strip_prefix(path_prefix).unwrap();
            let _path_rest_parent =
                path_rest.parent().unwrap_or_else(|| Path::new("")).join("");
            let _path_rest_name = path_rest.file_name().unwrap();

            stdout.set_color(ColorSpec::new().set_dimmed(true))?;
            write!(stdout, "{}", path_prefix.to_string_lossy())?;

            stdout.set_color(
                ColorSpec::new().set_fg(Some(color)).set_bold(true),
            )?;
            write!(stdout, "{}", path_rest.to_string_lossy())?;
        }
    }

    stdout.set_color(&ColorSpec::new())?;
    writeln!(stdout)?;
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
