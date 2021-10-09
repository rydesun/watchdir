mod cli;
mod inotify;
mod watcher;

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
        println!("{:?}", event);
    }
}

fn init_logger(verbose_level: i32) {
    let subscriber = tracing_subscriber::fmt();
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
