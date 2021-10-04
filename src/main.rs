mod cli;
mod inotify;
mod watcher;

fn main() {
    let opts = cli::parse();

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
