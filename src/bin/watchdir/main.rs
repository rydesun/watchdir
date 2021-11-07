mod cli;

use std::{io::Write, path::Path, time};

use mimalloc::MiMalloc;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use tracing::{error, info, warn, Level};
use tracing_subscriber::EnvFilter;
use watchdir::{Event, Watcher, WatcherOpts};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    let opts = cli::parse();

    init_logger(opts.debug, match opts.color {
        cli::ColorWhen::Always => true,
        cli::ColorWhen::Auto => isatty_stderr(),
        cli::ColorWhen::Never => false,
    });
    info!("version: {}", *cli::VERSION);

    info!("Initializing...");
    let now = time::Instant::now();
    let watcher = match Watcher::new(
        opts.dir.as_ref().unwrap(),
        WatcherOpts::new(
            if opts.include_hidden {
                watchdir::Dotdir::Include
            } else {
                watchdir::Dotdir::Exclude
            },
            opts.extra_events.into_iter().map(|e| e.into()).collect(),
        ),
    ) {
        Ok(watcher) => watcher,
        Err(e) => {
            error!("{}", e);
            std::process::exit(1);
        }
    };
    info!("Initialized successfully! Elapsed time: {:?}", now.elapsed());

    let mut stdout = StandardStream::stdout((&opts.color).into());
    for event in watcher {
        print_event(
            &mut stdout,
            &event,
            opts.dir.as_ref().unwrap(),
            opts.prefix,
        )
        .unwrap();
        match event {
            Event::MoveTop(_) => {
                warn!(
                    "Watched dir was moved. The prefix of path can no longer \
                     be trusted!"
                );
            }
            Event::DeleteTop(_) => {
                warn!("Watched dir was deleted.");
                std::process::exit(0);
            }
            Event::UnmountTop(_) => {
                warn!("Watched dir was unmounted.");
                std::process::exit(0);
            }
            _ => {}
        }
    }
}

fn print_event(
    stdout: &mut StandardStream,
    event: &Event,
    path_prefix: &Path,
    need_prefix: bool,
) -> Result<(), std::io::Error> {
    let (head, path, color) = match event {
        Event::Create(path) => ("Create", Some(path), Color::Green),
        Event::DeleteDir(path) => ("Delete", Some(path), Color::Magenta),
        Event::DeleteFile(path) => ("Delete", Some(path), Color::Magenta),
        Event::MoveDir(..) => ("Move", None, Color::Blue),
        Event::MoveFile(..) => ("Move", None, Color::Blue),
        Event::MoveAwayDir(path) => ("MoveAway", Some(path), Color::Blue),
        Event::MoveAwayFile(path) => ("MoveAway", Some(path), Color::Blue),
        Event::MoveInto(path) => ("MoveInto", Some(path), Color::Blue),
        Event::Modify(path) => ("Modify", Some(path), Color::Yellow),
        Event::Open(path) => ("Open", Some(path), Color::Cyan),
        Event::OpenTop(_) => ("Open", None, Color::Cyan),
        Event::Close(path) => ("Close", Some(path), Color::Cyan),
        Event::CloseTop(_) => ("Close", None, Color::Cyan),
        Event::Access(path) => ("Access", Some(path), Color::Cyan),
        Event::AccessTop(_) => ("Access", None, Color::Cyan),
        Event::Attrib(path) => ("Attrib", Some(path), Color::Yellow),
        Event::AttribTop(_) => ("Attrib", None, Color::Yellow),
        Event::MoveTop(path) => ("MoveTop", Some(path), Color::Red),
        Event::DeleteTop(path) => ("DeleteTop", Some(path), Color::Red),
        Event::Unmount(path) => ("Unmount", Some(path), Color::Magenta),
        Event::UnmountTop(path) => ("UnmountTop", Some(path), Color::Red),
        Event::Unknown => ("Unknown", None, Color::Red),
        Event::Ignored => return Ok(()),
        Event::Noise => panic!("Noise should never leak"),
    };

    write_color!(stdout, (color)[set_bold])?;
    write!(stdout, "{:<12}", head)?;

    match event {
        Event::MoveFile(from, to) | Event::MoveDir(from, to) => {
            let from_rest = from.strip_prefix(path_prefix).unwrap();
            let _from_rest_parent =
                from_rest.parent().unwrap_or_else(|| Path::new("")).join("");
            let _from_rest_name = from_rest.file_name().unwrap();
            let to_rest = to.strip_prefix(path_prefix).unwrap();
            let _to_rest_parent =
                to_rest.parent().unwrap_or_else(|| Path::new("")).join("");
            let _to_rest_name = to_rest.file_name().unwrap();

            if need_prefix {
                write_color!(stdout, [set_dimmed])?;
                write!(stdout, "{}", path_prefix.to_string_lossy())?;
            }

            write_color!(stdout, (color)[set_bold])?;
            write!(stdout, "{}", from_rest.to_string_lossy())?;

            write_color!(stdout, [set_dimmed])?;
            write!(stdout, " -> ")?;

            if need_prefix {
                write_color!(stdout, [set_dimmed])?;
                write!(stdout, "{}", path_prefix.to_string_lossy())?;
            }

            write_color!(stdout, (color)[set_bold])?;
            write!(stdout, "{}", to_rest.to_string_lossy())?;
        }
        Event::MoveTop(path)
        | Event::DeleteTop(path)
        | Event::UnmountTop(path)
        | Event::AccessTop(path)
        | Event::AttribTop(path)
        | Event::OpenTop(path)
        | Event::CloseTop(path) => {
            write_color!(stdout, reset)?;
            write!(stdout, "{}", path.to_string_lossy())?;
        }
        _ => {
            let path = path.unwrap();
            let path_rest = path.strip_prefix(path_prefix).unwrap();
            let _path_rest_parent =
                path_rest.parent().unwrap_or_else(|| Path::new("")).join("");
            let _path_rest_name = path_rest.file_name().unwrap();

            if need_prefix {
                write_color!(stdout, [set_dimmed])?;
                write!(stdout, "{}", path_prefix.to_string_lossy())?;
            }

            write_color!(stdout, (color)[set_bold])?;
            write!(stdout, "{}", path_rest.to_string_lossy())?;
        }
    }

    write_color!(stdout, reset)?;
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
            .pretty()
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

impl From<cli::ExtraEvent> for watchdir::ExtraEvent {
    fn from(v: cli::ExtraEvent) -> Self {
        match v {
            cli::ExtraEvent::Modify => watchdir::ExtraEvent::Modify,
            cli::ExtraEvent::Attrib => watchdir::ExtraEvent::Attrib,
            cli::ExtraEvent::Access => watchdir::ExtraEvent::Access,
            cli::ExtraEvent::Open => watchdir::ExtraEvent::Open,
            cli::ExtraEvent::Close => watchdir::ExtraEvent::Close,
        }
    }
}

impl From<&cli::ColorWhen> for ColorChoice {
    fn from(v: &cli::ColorWhen) -> Self {
        match v {
            cli::ColorWhen::Always => Self::AlwaysAnsi,
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

#[macro_export]
macro_rules! write_color {
    ( $writer:expr, reset ) => {
        $writer.set_color(&ColorSpec::new())
    };

    (
        $writer:expr,
        $( (
            $( $fg:expr )? $( ,$bg:expr )?
        ) )?
        [
            $( $effect:ident ),*
        ]
    ) => {
        $writer.set_color(ColorSpec::new()
            $(
                $(.set_fg(Some($fg)))?
                $(.set_bg(Some($bg)))?
            )?
            $(.$effect(true))*)
    };
}
