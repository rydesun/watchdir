mod cli;
mod inotify;
mod watcher;

use tracing::Level;
use tracing_subscriber::EnvFilter;

fn main() {
    let opts = cli::parse();

    init_logger(opts.verbose);

    let watcher = match watcher::Watcher::new(
        &opts.dirs,
        if opts.hidden {
            watcher::Dotdir::Include
        } else {
            watcher::Dotdir::Exclude
        },
    ) {
        Ok(watcher) => watcher,
        Err(_) => {
            return;
        }
    };
    for event in watcher {
        println!("{:?}", event);
    }
}

fn init_logger(verbose_level: i32) {
    let subscriber = tracing_subscriber::fmt();
    match verbose_level {
        0 => subscriber
            .with_env_filter(EnvFilter::new(Level::WARN.to_string()))
            .init(),
        1 => subscriber.init(),
        _ => subscriber.pretty().init(),
    };
}
