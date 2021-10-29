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

    let mut color_spec = ColorSpec::new();
    let (color_choice, log_color) = match opts.color {
        cli::ColorWhen::Always => (ColorChoice::Always, true),
        cli::ColorWhen::Ansi => (ColorChoice::AlwaysAnsi, true),
        cli::ColorWhen::Auto => (
            if isatty_stdout() {
                ColorChoice::Auto
            } else {
                ColorChoice::Never
            },
            isatty_stderr(),
        ),
        _ => (ColorChoice::Never, false),
    };
    let mut stdout = StandardStream::stdout(color_choice);

    init_logger(opts.verbose, log_color);
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
            watcher::Event::MoveTop(_) => {
                warn!(
                    "Watched dir was moved. The prefix of path can no longer \
                     be trusted!"
                );
            }
            watcher::Event::DeleteTop(_) => {
                print_event(&mut stdout, &mut color_spec, event).unwrap();
                warn!("Watched dir was deleted.");
                std::process::exit(0);
            }
            watcher::Event::Ignored => continue,
            _ => {}
        }
        print_event(&mut stdout, &mut color_spec, event).unwrap();
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
            ("Delete", format!("{:?}", path), Color::Magenta)
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
        watcher::Event::MoveTop(path) => {
            ("MoveTop", format!("{:?}", path), Color::Red)
        }
        watcher::Event::DeleteTop(path) => {
            ("DeleteTop", format!("{:?}", path), Color::Red)
        }
        _ => ("Unknown", "".to_owned(), Color::Red),
    };
    stdout.set_color(color_spec.set_fg(Some(color)))?;
    write!(stdout, "{:<12}", head)?;
    writeln!(stdout, "{}", content)?;
    Ok(())
}

fn init_logger(verbose_level: i32, color: bool) {
    let subscriber = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(color);
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

fn isatty_stdout() -> bool {
    unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 }
}

fn isatty_stderr() -> bool {
    unsafe { libc::isatty(libc::STDERR_FILENO) != 0 }
}
