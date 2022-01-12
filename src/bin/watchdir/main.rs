#[cfg(not(target_os = "linux"))]
compile_error!("This program only works on Linux.");

mod cli;
mod print;
mod theme;

use futures::{pin_mut, StreamExt};
use termcolor::ColorChoice;
use tokio::sync::mpsc;
use tracing::{error, info, warn, Level};
use tracing_subscriber::EnvFilter;
use watchdir::{Event, Watcher, WatcherOpts};

#[tokio::main]
async fn main() {
    let opts = cli::parse();

    init_logger(opts.debug, match opts.color {
        cli::ColorWhen::Always => true,
        cli::ColorWhen::Auto => isatty_stderr(),
        cli::ColorWhen::Never => false,
    });

    let dirs = directories::ProjectDirs::from("", "", env!("CARGO_BIN_NAME"))
        .unwrap();
    let file_theme = dirs.config_dir().join("theme.yaml");
    let printer_theme = std::fs::File::open(file_theme)
        .map(|f| {
            let res: Result<theme::Theme, serde_yaml::Error> =
                serde_yaml::from_reader(f);
            match res {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to parse theme config: {}", e);
                    std::process::exit(1);
                }
            }
        })
        .unwrap_or_else(|e| {
            if e.kind() != std::io::ErrorKind::NotFound {
                error!("Failed to read theme config: {}", e);
                std::process::exit(1);
            }
            theme::Theme::default()
        });

    info!("version: {}", *cli::VERSION);
    info!("Initializing...");
    let now = std::time::Instant::now();
    let mut watcher = match Watcher::new(
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

    let (tx, mut rx) = mpsc::channel(32);
    tokio::spawn(async move {
        let event_stream = watcher.stream();
        pin_mut!(event_stream);
        while let Some(event) = event_stream.next().await {
            tx.send(event).await.unwrap();
        }
    });

    let mut printer = print::Printer::new(print::PrinterOpts {
        need_ansi: match opts.color {
            cli::ColorWhen::Always => true,
            cli::ColorWhen::Auto => isatty_stdout(),
            cli::ColorWhen::Never => false,
        },
        color_choice: (&opts.color).into(),
        theme: printer_theme,
        top_dir: opts.dir.unwrap().to_owned(),
        need_time: opts.time,
        need_prefix: opts.prefix,
        timeout_modify: std::time::Duration::from_millis(opts.throttle_modify),
        event_filter: opts
            .exclude_events
            .into_iter()
            .map(|v| v.into())
            .collect(),
    });

    loop {
        let (event, t) = rx.recv().await.unwrap();
        printer.print(&event, t).unwrap();
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
            Event::Unknown => {
                error!("Unknown event occurs.");
            }
            Event::Noise => {
                error!("Noise event should never leak.");
            }
            _ => {}
        }
    }
}

fn init_logger(debug: bool, color: bool) {
    let time_format = time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:\
         [second]+[offset_hour][offset_minute]"
    );

    let subscriber = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(color);

    #[cfg(unsound_local_offset)]
    let subscriber = subscriber.with_timer(
        tracing_subscriber::fmt::time::LocalTime::new(time_format),
    );
    #[cfg(not(unsound_local_offset))]
    let subscriber = subscriber
        .with_timer(tracing_subscriber::fmt::time::UtcTime::new(time_format));

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

impl From<cli::Event> for print::EventGroup {
    fn from(v: cli::Event) -> Self {
        match v {
            cli::Event::Create => print::EventGroup::Create,
            cli::Event::Delete => print::EventGroup::Delete,
            cli::Event::Move => print::EventGroup::Move,
            cli::Event::Unmount => print::EventGroup::Unmount,
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
