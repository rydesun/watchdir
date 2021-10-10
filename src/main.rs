mod cli;
mod inotify;
mod watcher;

use std::io::Write;

use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use tracing::{error, info, Level};
use tracing_subscriber::EnvFilter;

fn main() {
    let opts = match cli::parse() {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    init_logger(opts.verbose);

    let mut color_spec = ColorSpec::new();
    let color_choice = match opts.color {
        cli::ColorWhen::Always => ColorChoice::Always,
        cli::ColorWhen::Ansi => ColorChoice::AlwaysAnsi,
        cli::ColorWhen::Auto => {
            if isatty() {
                ColorChoice::Auto
            } else {
                ColorChoice::Never
            }
        }
        _ => ColorChoice::Never,
    };
    let mut stdout = StandardStream::stdout(color_choice);

    info!("version: {}", cli::VERSION);

    let watcher = match watcher::Watcher::new(
        &opts.dir,
        if opts.hidden {
            watcher::Dotdir::Include
        } else {
            watcher::Dotdir::Exclude
        },
    ) {
        Ok(watcher) => watcher,
        Err(e) => {
            error!("{}", e);
            std::process::exit(1);
        }
    };
    info!("initialized successfully and listening to upcoming events...\n");

    for event in watcher {
        if event == watcher::Event::Ignored {
            continue;
        }
        print_event(&mut stdout, &mut color_spec, event).unwrap()
    }
}

fn print_event(
    stdout: &mut StandardStream,
    color_spec: &mut ColorSpec,
    event: watcher::Event,
) -> Result<(), std::io::Error> {
    match event {
        watcher::Event::Create(path) => {
            stdout.set_color(color_spec.set_fg(Some(Color::Green)))?;
            write!(stdout, "{:<12}", "Create")?;
            writeln!(stdout, "{:?}", path)?;
        }
        watcher::Event::Delete(path) => {
            stdout.set_color(color_spec.set_fg(Some(Color::Red)))?;
            write!(stdout, "{:<12}", "Delete")?;
            writeln!(stdout, "{:?}", path)?;
        }
        watcher::Event::Move(from, to) => {
            stdout.set_color(color_spec.set_fg(Some(Color::Blue)))?;
            write!(stdout, "{:<12}", "Move")?;
            writeln!(stdout, "{:?} -> {:?}", from, to)?;
        }
        watcher::Event::MoveAway(path) => {
            stdout.set_color(color_spec.set_fg(Some(Color::Blue)))?;
            write!(stdout, "{:<12}", "MoveAway")?;
            writeln!(stdout, "{:?}", path)?;
        }
        watcher::Event::MoveInto(path) => {
            stdout.set_color(color_spec.set_fg(Some(Color::Blue)))?;
            write!(stdout, "{:<12}", "MoveInto")?;
            writeln!(stdout, "{:?}", path)?;
        }
        _ => {
            stdout.set_color(color_spec.set_fg(Some(Color::Red)))?;
            writeln!(stdout, "Unknown")?;
        }
    }
    Ok(())
}

fn init_logger(verbose_level: i32) {
    let subscriber = tracing_subscriber::fmt().with_writer(std::io::stderr);
    match verbose_level {
        0 => subscriber.init(),
        1 => subscriber
            .with_env_filter(EnvFilter::new(Level::DEBUG.to_string()))
            .init(),
        _ => subscriber
            .pretty()
            .with_env_filter(EnvFilter::new(Level::DEBUG.to_string()))
            .init(),
    };
}

fn isatty() -> bool {
    unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 }
}
