use std::{fs, path::PathBuf};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use watchdir::{Dotdir, Watcher, WatcherOpts};

pub fn bench_watch_dir_with_subdirs(c: &mut Criterion) {
    let mut group = c.benchmark_group("Watch with subdirs");

    let opts = WatcherOpts::new(Dotdir::Exclude, false);

    for i in (0..=100).step_by(20) {
        let top_dir_with_subdirs = tempfile::tempdir().unwrap();
        let mut sub_dirs = PathBuf::new();
        for _ in 0..i {
            sub_dirs.push(random_string(5));
        }
        fs::create_dir_all(&top_dir_with_subdirs.path().join(&sub_dirs))
            .unwrap();

        group.bench_function(BenchmarkId::new("depth", i), |b| {
            b.iter(|| Watcher::new(top_dir_with_subdirs.path(), opts))
        });
    }
    group.finish()
}

pub fn bench_move_dir_with_subdirs(c: &mut Criterion) {
    let mut group = c.benchmark_group("Move with subdirs");

    let opts = WatcherOpts::new(Dotdir::Exclude, false);

    let tempdir = tempfile::tempdir().unwrap();
    let dest_tempdir = tempdir.path().join(random_string(5));

    for i in (0..=100).step_by(20) {
        let top_dir_with_subdirs = tempfile::tempdir().unwrap();
        let mut sub_dirs = PathBuf::new();
        for _ in 0..i {
            sub_dirs.push(random_string(5));
        }
        fs::create_dir_all(&top_dir_with_subdirs.path().join(&sub_dirs))
            .unwrap();

        let watcher = Watcher::new(top_dir_with_subdirs.path(), opts);

        group.bench_function(BenchmarkId::new("depth", i), |b| {
            b.iter(|| {
                fs::rename(&top_dir_with_subdirs.path(), &dest_tempdir)
                    .unwrap();
                watcher.iter().next().unwrap();
                fs::rename(&dest_tempdir, &top_dir_with_subdirs.path())
                    .unwrap();
            });
        });
    }
    group.finish()
}

fn random_string(len: usize) -> String {
    thread_rng().sample_iter(&Alphanumeric).take(len).map(char::from).collect()
}

criterion_group!(
    benches,
    bench_watch_dir_with_subdirs,
    bench_move_dir_with_subdirs,
);
criterion_main!(benches);
