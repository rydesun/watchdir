mod cli;
mod inotify;
mod path_tree;
mod watcher;

use std::io::Write;

use mimalloc::MiMalloc;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use tracing::{error, info, warn, Level};
use tracing_subscriber::EnvFilter;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

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
        watcher::WatcherOpts::new(
            if opts.hidden {
                watcher::Dotdir::Include
            } else {
                watcher::Dotdir::Exclude
            },
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
        match event {
            watcher::Event::MoveTop => {
                warn!(
                    "Watched dir was moved. The prefix of path can no longer \
                     be trusted!"
                );
            }
            watcher::Event::DeleteTop => {
                warn!("Watched dir was deleted.");
                std::process::exit(0);
            }
            watcher::Event::Ignored => continue,
            _ => print_event(&mut stdout, &mut color_spec, event).unwrap(),
        }
    }
}

fn print_event(
    stdout: &mut StandardStream,
    color_spec: &mut ColorSpec,
    event: watcher::Event,
) -> Result<(), std::io::Error> {
    let (head, content, color) = match event {
        watcher::Event::Create(path) => {
            ("Create", format!("{:?}", path), Color::Green)
        }
        watcher::Event::Delete(path) => {
            ("Delete", format!("{:?}", path), Color::Red)
        }
        watcher::Event::Move(from, to) => {
            ("Move", format!("{:?} -> {:?}", from, to), Color::Blue)
        }
        watcher::Event::MoveAway(path) => {
            ("MoveAway", format!("{:?}", path), Color::Blue)
        }
        watcher::Event::MoveInto(path) => {
            ("MoveInto", format!("{:?}", path), Color::Blue)
        }
        watcher::Event::Modify(path) => {
            ("Modify", format!("{:?}", path), Color::Yellow)
        }
        _ => ("Unknown", "".to_owned(), Color::Red),
    };
    stdout.set_color(color_spec.set_fg(Some(color)))?;
    write!(stdout, "{:<12}", head)?;
    writeln!(stdout, "{}", content)?;
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
