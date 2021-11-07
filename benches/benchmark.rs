use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::Stdio,
};

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

pub fn bench_program_init(c: &mut Criterion) {
    let mut group = c.benchmark_group("Program init");

    for i in (0..=100).step_by(20) {
        let top_dir_with_subdirs = tempfile::tempdir().unwrap();
        let mut sub_dirs = PathBuf::new();
        for _ in 0..i {
            sub_dirs.push(random_string(5));
        }
        let deepest_dir = &top_dir_with_subdirs.path().join(&sub_dirs);
        fs::create_dir_all(deepest_dir).unwrap();
        for i in deepest_dir
            .ancestors()
            .take_while(|d| *d != top_dir_with_subdirs.path())
        {
            for _ in 0..=100 {
                fs::File::create(i.join(random_string(5))).unwrap();
            }
        }

        group.bench_function(BenchmarkId::new("watchdir", i), |b| {
            b.iter(|| {
                let mut p =
                    std::process::Command::new(env!("CARGO_BIN_EXE_watchdir"))
                        .arg("--include-hidden")
                        .arg(top_dir_with_subdirs.path())
                        .stderr(Stdio::piped())
                        .spawn()
                        .unwrap();
                let mut stderr =
                    BufReader::new(p.stderr.as_mut().unwrap()).lines().skip(1);
                assert!(stderr
                    .next()
                    .unwrap()
                    .unwrap()
                    .contains("Initializing..."));
                assert!(stderr
                    .next()
                    .unwrap()
                    .unwrap()
                    .contains("Initialized"));
                p.kill()
            })
        });

        group.bench_function(BenchmarkId::new("inotifywait", i), |b| {
            b.iter(|| {
                let mut p = std::process::Command::new("inotifywait")
                    .arg("--monitor")
                    .arg("--recursive")
                    .arg(top_dir_with_subdirs.path())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                let mut stderr =
                    BufReader::new(p.stderr.as_mut().unwrap()).lines();
                assert!(stderr
                    .next()
                    .unwrap()
                    .unwrap()
                    .contains("Setting up watches"));
                assert!(stderr
                    .next()
                    .unwrap()
                    .unwrap()
                    .contains("Watches established"));
                p.kill()
            })
        });
    }
    group.finish()
}

pub fn bench_program_watch_move_dir(c: &mut Criterion) {
    let mut group = c.benchmark_group("Program watch move");

    for i in (0..=100).step_by(20) {
        let topdir = tempfile::tempdir().unwrap();
        let from_tempdir = topdir.path().join(random_string(5));
        let dest_tempdir = topdir.path().join(random_string(5));
        let mut sub_dirs = PathBuf::new();
        for _ in 0..i {
            sub_dirs.push(random_string(5));
        }
        let deepest_dir = from_tempdir.join(&sub_dirs);
        fs::create_dir_all(&deepest_dir).unwrap();
        for i in deepest_dir.ancestors().take_while(|d| *d != topdir.path()) {
            for _ in 0..=100 {
                fs::File::create(i.join(random_string(5))).unwrap();
            }
        }

        group.bench_function(BenchmarkId::new("watchdir", i), |b| {
            let mut p =
                std::process::Command::new(env!("CARGO_BIN_EXE_watchdir"))
                    .arg("--include-hidden")
                    .arg(topdir.path())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
            let mut stdout =
                BufReader::new(p.stdout.as_mut().unwrap()).lines();
            let stderr = BufReader::new(p.stderr.as_mut().unwrap()).lines();
            for line in stderr {
                if line.unwrap().contains("Initialized") {
                    break;
                }
            }

            b.iter(|| {
                fs::rename(&from_tempdir, &dest_tempdir).unwrap();
                assert!(stdout.next().unwrap().unwrap().contains("Move "));
                fs::rename(&dest_tempdir, &from_tempdir).unwrap();
                assert!(stdout.next().unwrap().unwrap().contains("Move "));
            });
            p.kill().unwrap()
        });

        group.bench_function(BenchmarkId::new("inotifywait", i), |b| {
            let mut p = std::process::Command::new("inotifywait")
                .arg("--monitor")
                .arg("--recursive")
                .arg(topdir.path())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let mut stdout =
                BufReader::new(p.stdout.as_mut().unwrap()).lines();
            let stderr = BufReader::new(p.stderr.as_mut().unwrap()).lines();
            for line in stderr {
                if line.unwrap().contains("Watches established") {
                    break;
                }
            }

            b.iter(|| {
                fs::rename(&from_tempdir, &dest_tempdir).unwrap();
                assert!(stdout
                    .next()
                    .unwrap()
                    .unwrap()
                    .contains("MOVED_FROM"));
                assert!(stdout.next().unwrap().unwrap().contains("MOVED_TO"));
                assert!(stdout.next().unwrap().unwrap().contains("MOVE_SELF"));
                fs::rename(&dest_tempdir, &from_tempdir).unwrap();
                assert!(stdout
                    .next()
                    .unwrap()
                    .unwrap()
                    .contains("MOVED_FROM"));
                assert!(stdout.next().unwrap().unwrap().contains("MOVED_TO"));
                assert!(stdout.next().unwrap().unwrap().contains("MOVE_SELF"));
            });
            p.kill().unwrap()
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
    bench_program_init,
    bench_program_watch_move_dir,
);
criterion_main!(benches);
