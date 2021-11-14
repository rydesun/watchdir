use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use criterion::{
    criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup,
    BenchmarkId, Criterion,
};
use rand::{distributions::Alphanumeric, thread_rng, Rng};

pub fn bench_init_dir_with_shallow_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("Program init dir with shallow files");
    group
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(1));
    bench_init(
        &mut group,
        setup_tempdir_with_shallow_files,
        &mut (0..100).step_by(20).chain((100..=1000).step_by(100)),
    );
    group.finish()
}

pub fn bench_init_dir_with_shallow_subdirs(c: &mut Criterion) {
    let mut group = c.benchmark_group("Program init dir with shallow subdirs");
    group
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    bench_init(
        &mut group,
        setup_tempdir_with_shallow_subdirs,
        &mut (0..100).step_by(20).chain((100..=1000).step_by(100)),
    );
    group.finish()
}

pub fn bench_init_dir_with_deep_subdirs(c: &mut Criterion) {
    let mut group = c.benchmark_group("Program init dir with deep subdirs");
    group
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(1));
    bench_init(
        &mut group,
        setup_tempdir_with_deep_subdirs,
        &mut (0..=50).step_by(5),
    );
    group.finish()
}

pub fn bench_init(
    group: &mut BenchmarkGroup<'_, WallTime>,
    setup_tempdir: fn(&Path, u32),
    iterator: &mut dyn Iterator<Item = u32>,
) {
    iterator.for_each(|count| {
        let top_dir = tempfile::tempdir().unwrap();
        let top_dir = top_dir.path();
        setup_tempdir(top_dir, count);

        let mut bin_watchdir = Command::new(env!("CARGO_BIN_EXE_watchdir"));
        let exec_watchdir = bin_watchdir
            .arg(top_dir)
            .arg("--include-hidden")
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut bin_inotifywait = Command::new("inotifywait");
        let exec_inotifywait = bin_inotifywait
            .arg(top_dir)
            .arg("--monitor")
            .arg("--recursive")
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut bin_fswatch = Command::new("fswatch");
        let exec_fswatch = bin_fswatch
            .arg(top_dir)
            .arg("--verbose")
            .arg("--recursive")
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        group.bench_function(BenchmarkId::new("watchdir", count), |b| {
            b.iter_custom(|iters| {
                let mut total = Duration::default();
                for _i in 0..iters {
                    let start = Instant::now();
                    let mut exec_watchdir = exec_watchdir.spawn().unwrap();
                    let mut stderr =
                        BufReader::new(exec_watchdir.stderr.as_mut().unwrap())
                            .lines()
                            .skip(1);
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
                    total += start.elapsed();
                    exec_watchdir.kill().unwrap();
                    exec_watchdir.wait().unwrap();
                }
                total
            })
        });

        group.bench_function(BenchmarkId::new("inotifywait", count), |b| {
            b.iter_custom(|iters| {
                let mut total = Duration::default();
                for _i in 0..iters {
                    let start = Instant::now();
                    let mut exec_inotifywait =
                        exec_inotifywait.spawn().unwrap();
                    let mut stderr = BufReader::new(
                        exec_inotifywait.stderr.as_mut().unwrap(),
                    )
                    .lines();
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
                    total += start.elapsed();
                    exec_inotifywait.kill().unwrap();
                    exec_inotifywait.wait().unwrap();
                }
                total
            })
        });

        group.bench_function(BenchmarkId::new("fswatch", count), |b| {
            b.iter_custom(|iters| {
                let mut total = Duration::default();
                for _i in 0..iters {
                    let start = Instant::now();
                    let mut exec_fswatch = exec_fswatch.spawn().unwrap();
                    let stderr =
                        BufReader::new(exec_fswatch.stderr.as_mut().unwrap())
                            .lines();
                    for line in stderr {
                        if line.unwrap().contains("run: Number of records:") {
                            break;
                        }
                    }
                    total += start.elapsed();
                    exec_fswatch.kill().unwrap();
                    exec_fswatch.wait().unwrap();
                }
                total
            })
        });
    });
}

pub fn bench_move_dir_with_shallow_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("Program move dir with shallow files");
    group
        .warm_up_time(Duration::from_millis(100))
        .measurement_time(Duration::from_millis(400));
    bench_move_dir(
        &mut group,
        setup_tempdir_with_shallow_files,
        &mut (0..100).step_by(20).chain((100..=1000).step_by(100)),
    );
    group.finish()
}

pub fn bench_move_dir_with_shallow_subdirs(c: &mut Criterion) {
    let mut group = c.benchmark_group("Program move dir with shallow subdirs");
    group
        .warm_up_time(Duration::from_millis(100))
        .measurement_time(Duration::from_millis(400));
    bench_move_dir(
        &mut group,
        setup_tempdir_with_shallow_subdirs,
        &mut (0..100).step_by(20).chain((100..=1000).step_by(100)),
    );
    group.finish()
}

pub fn bench_move_dir_with_deep_subdirs(c: &mut Criterion) {
    let mut group = c.benchmark_group("Program move dir with deep subdirs");
    group
        .warm_up_time(Duration::from_millis(100))
        .measurement_time(Duration::from_millis(400));
    bench_move_dir(
        &mut group,
        setup_tempdir_with_deep_subdirs,
        &mut (0..=50).step_by(5),
    );
    group.finish()
}

pub fn bench_move_dir(
    group: &mut BenchmarkGroup<'_, WallTime>,
    setup_tempdir: fn(&Path, u32),
    iterator: &mut dyn Iterator<Item = u32>,
) {
    iterator.for_each(|count| {
        let top_dir = tempfile::tempdir().unwrap();
        let top_dir = top_dir.path();
        let from_tempdir = top_dir.join(random_string(5));
        fs::create_dir(&from_tempdir).unwrap();
        setup_tempdir(&from_tempdir, count);
        let dest_tempdir = top_dir.join(random_string(5));

        let mut bin_watchdir = Command::new(env!("CARGO_BIN_EXE_watchdir"));
        let exec_watchdir = bin_watchdir
            .arg(top_dir)
            .arg("--include-hidden")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut bin_inotifywait = Command::new("inotifywait");
        let exec_inotifywait = bin_inotifywait
            .arg(top_dir)
            .arg("--monitor")
            .arg("--recursive")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        group.bench_function(BenchmarkId::new("watchdir", count), |b| {
            let mut exec_watchdir = exec_watchdir.spawn().unwrap();
            let mut stdout =
                BufReader::new(exec_watchdir.stdout.as_mut().unwrap()).lines();
            let stderr =
                BufReader::new(exec_watchdir.stderr.as_mut().unwrap()).lines();
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
            exec_watchdir.kill().unwrap();
            exec_watchdir.wait().unwrap();
        });

        group.bench_function(BenchmarkId::new("inotifywait", count), |b| {
            let mut exec_inotifywait = exec_inotifywait.spawn().unwrap();
            let mut stdout =
                BufReader::new(exec_inotifywait.stdout.as_mut().unwrap())
                    .lines();
            let stderr =
                BufReader::new(exec_inotifywait.stderr.as_mut().unwrap())
                    .lines();
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
            exec_inotifywait.kill().unwrap();
            exec_inotifywait.wait().unwrap();
        });

        // fswatch takes too much times, so there is no need to bench it.
    });
}

fn setup_tempdir_with_shallow_files(tempdir: &Path, count: u32) {
    (0..count).for_each(|_| {
        fs::File::create(tempdir.join(random_string(5))).unwrap();
    });
}

fn setup_tempdir_with_shallow_subdirs(tempdir: &Path, count: u32) {
    (0..count).for_each(|_| {
        fs::create_dir(tempdir.join(random_string(5))).unwrap();
    });
}

fn setup_tempdir_with_deep_subdirs(tempdir: &Path, depth: u32) {
    let mut subdirs = PathBuf::new();
    (0..depth).for_each(|_| {
        subdirs.push(random_string(5));
    });
    let bottom_dir = tempdir.join(&subdirs);
    fs::create_dir_all(bottom_dir).unwrap();
}

fn random_string(len: usize) -> String {
    thread_rng().sample_iter(&Alphanumeric).take(len).map(char::from).collect()
}

criterion_group!(
    benches,
    bench_init_dir_with_shallow_files,
    bench_init_dir_with_shallow_subdirs,
    bench_init_dir_with_deep_subdirs,
    bench_move_dir_with_shallow_files,
    bench_move_dir_with_shallow_subdirs,
    bench_move_dir_with_deep_subdirs,
);
criterion_main!(benches);
