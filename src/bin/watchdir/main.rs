mod cli;
mod print;
mod theme;

use std::time;

use futures::{pin_mut, StreamExt};
use mimalloc::MiMalloc;
use termcolor::{ColorChoice, StandardStream};
use tokio::sync::mpsc;
use tracing::{error, info, warn, Level};
use tracing_subscriber::EnvFilter;
use watchdir::{Event, Watcher, WatcherOpts};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() {
    let opts = cli::parse();

    init_logger(opts.debug, match opts.color {
        cli::ColorWhen::Always => true,
        cli::ColorWhen::Auto => isatty_stderr(),
        cli::ColorWhen::Never => false,
    });

    info!("version: {}", *cli::VERSION);
    info!("Initializing...");
    let now = time::Instant::now();
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

    let mut printer = print::Printer::new(
        StandardStream::stdout((&opts.color).into()),
        theme::Theme {},
        opts.dir.unwrap().to_owned(),
        opts.prefix,
        opts.throttle_modify,
    );

    loop {
        let event = rx.recv().await.unwrap();
        printer.print(&event).unwrap();
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
