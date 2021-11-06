use std::{
    fs::{self, File},
    path::PathBuf,
};

use rand::{distributions::Alphanumeric, thread_rng, Rng};
use watchdir::*;

fn random_string(len: usize) -> String {
    thread_rng().sample_iter(&Alphanumeric).take(len).map(char::from).collect()
}

#[test]
fn test_create_file() {
    let top_dir = tempfile::tempdir().unwrap();
    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let path = top_dir.path().join(random_string(5));
    File::create(&path).unwrap();
    assert_eq!(watcher.next().unwrap(), Event::Create(path))
}

#[test]
fn test_create_in_created_subdir() {
    let top_dir = tempfile::tempdir().unwrap();
    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let dir = top_dir.path().join(random_string(5));
    fs::create_dir(&dir).unwrap();
    assert_eq!(watcher.next().unwrap(), Event::Create(dir.to_owned()));

    let path = dir.join(random_string(5));
    File::create(&path).unwrap();
    assert_eq!(watcher.next().unwrap(), Event::Create(path))
}

#[test]
fn test_create_in_recur_created_subdir() {
    let top_dir = tempfile::tempdir().unwrap();
    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let recur_depth = 3;
    let mut dir = top_dir.path().to_owned();
    let mut dirs: Vec<PathBuf> = Vec::<PathBuf>::new();
    for _ in 0..recur_depth {
        dir = dir.join(random_string(5));
        dirs.push(dir.to_owned());
    }
    fs::create_dir_all(&dir).unwrap();
    for d in dirs.iter().take(recur_depth) {
        assert_eq!(watcher.next().unwrap(), Event::Create(d.to_owned()));
    }

    let path = dir.join(random_string(5));
    File::create(&path).unwrap();
    assert_eq!(watcher.next().unwrap(), Event::Create(path))
}

#[test]
fn test_move_dir() {
    let top_dir = tempfile::tempdir().unwrap();
    let old_dir = top_dir.path().join(random_string(5));
    fs::create_dir(&old_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let new_dir = top_dir.path().join(random_string(5));
    fs::rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();

    assert_eq!(watcher.next().unwrap(), Event::MoveDir(old_dir, new_dir))
}

#[test]
fn test_move_long_name_dir() {
    let top_dir = tempfile::tempdir().unwrap();
    let old_dir = top_dir.path().join(random_string(180));
    fs::create_dir(&old_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let new_dir = top_dir.path().join(random_string(180));
    fs::rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();

    assert_eq!(watcher.next().unwrap(), Event::MoveDir(old_dir, new_dir))
}

#[test]
fn test_move_top_dir() {
    let top_dir = tempfile::tempdir().unwrap();
    let top_dir = top_dir.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let new_top_dir = temp_dir.path().join(random_string(5));

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    fs::rename(&top_dir, new_top_dir).unwrap();

    assert_eq!(watcher.next().unwrap(), Event::MoveTop(top_dir))
}

#[test]
fn test_create_in_moved_subdir() {
    let top_dir = tempfile::tempdir().unwrap();

    let old_dir = top_dir.path().join(random_string(5));

    let mut sub_dirs = PathBuf::new();
    for _ in 0..3 {
        sub_dirs.push(PathBuf::from(random_string(5)));
    }
    fs::create_dir_all(&old_dir.join(sub_dirs.to_owned())).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let new_dir = top_dir.path().join(random_string(5));

    fs::rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();
    assert_eq!(
        watcher.next().unwrap(),
        Event::MoveDir(old_dir, new_dir.to_owned())
    );

    let new_file = new_dir.join(sub_dirs).join(random_string(5));
    File::create(&new_file).unwrap();
    assert_eq!(watcher.next().unwrap(), Event::Create(new_file))
}

#[test]
fn test_create_in_moved_dir_in_subdir() {
    let top_dir = tempfile::tempdir().unwrap();

    let old_dir = top_dir.path().join(random_string(5));
    fs::create_dir(&old_dir).unwrap();

    let mut sub_dirs = top_dir.path().to_owned();
    for _ in 0..3 {
        sub_dirs.push(PathBuf::from(random_string(5)));
    }
    fs::create_dir_all(&top_dir.path().join(sub_dirs.to_owned())).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let new_dir = sub_dirs.to_owned().join(random_string(5));
    fs::rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();
    assert_eq!(
        watcher.next().unwrap(),
        Event::MoveDir(old_dir, new_dir.to_owned())
    );

    let new_file = new_dir.join(random_string(5));
    File::create(&new_file).unwrap();
    assert_eq!(watcher.next().unwrap(), Event::Create(new_file))
}

#[test]
fn test_move_file() {
    let top_dir = tempfile::tempdir().unwrap();
    let old_file = top_dir.path().join(random_string(5));
    File::create(&old_file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let new_file = top_dir.path().join(random_string(5));
    fs::rename(old_file.to_owned(), new_file.to_owned()).unwrap();

    assert_eq!(watcher.next().unwrap(), Event::MoveFile(old_file, new_file))
}

#[test]
fn test_dir_move_away() {
    let top_dir = tempfile::tempdir().unwrap();
    let unwatched_dir = tempfile::tempdir().unwrap();
    let old_dir = top_dir.path().join(random_string(5));
    fs::create_dir(&old_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let new_dir = unwatched_dir.path().join(random_string(5));
    fs::rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();

    assert_eq!(watcher.next().unwrap(), Event::MoveAwayDir(old_dir));

    let unwatched_file = new_dir.join(random_string(5));
    File::create(&unwatched_file).unwrap();
    assert_eq!(watcher.next().unwrap(), Event::Ignored);
}

#[test]
fn test_file_move_away() {
    let top_dir = tempfile::tempdir().unwrap();
    let unwatched_dir = tempfile::tempdir().unwrap();
    let old_file = top_dir.path().join(random_string(5));
    File::create(&old_file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let new_file = unwatched_dir.path().join(random_string(5));
    fs::rename(old_file.to_owned(), new_file).unwrap();

    assert_eq!(watcher.next().unwrap(), Event::MoveAwayFile(old_file));
}

#[test]
fn test_dir_move_into() {
    let top_dir = tempfile::tempdir().unwrap();
    let unwatched_dir = tempfile::tempdir().unwrap();
    let old_dir = unwatched_dir.path().join(random_string(5));
    fs::create_dir(&old_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let new_dir = top_dir.path().join(random_string(5));
    fs::rename(old_dir, new_dir.to_owned()).unwrap();

    assert_eq!(watcher.next().unwrap(), Event::MoveInto(new_dir.to_owned()));

    let new_file = new_dir.join(random_string(5));
    File::create(&new_file).unwrap();
    assert_eq!(watcher.next().unwrap(), Event::Create(new_file));
}

#[test]
fn test_file_move_into() {
    let top_dir = tempfile::tempdir().unwrap();
    let unwatched_dir = tempfile::tempdir().unwrap();
    let old_file = unwatched_dir.path().join(random_string(5));
    File::create(&old_file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let new_file = top_dir.path().join(random_string(5));
    fs::rename(old_file, new_file.to_owned()).unwrap();

    assert_eq!(watcher.next().unwrap(), Event::MoveInto(new_file));
}

#[test]
fn test_file_move_away_and_move_into() {
    let top_dir = tempfile::tempdir().unwrap();
    let unwatched_dir = tempfile::tempdir().unwrap();

    let old_file = top_dir.path().join(random_string(5));
    File::create(&old_file).unwrap();

    let next_file_name = random_string(5);
    let next_old_file = unwatched_dir.path().join(next_file_name.to_owned());
    File::create(&next_old_file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    let new_file = unwatched_dir.path().join(random_string(5));
    fs::rename(old_file.to_owned(), new_file).unwrap();
    let next_new_file = top_dir.path().join(next_file_name);
    fs::rename(next_old_file, next_new_file.to_owned()).unwrap();

    assert_eq!(watcher.next().unwrap(), Event::MoveAwayFile(old_file));
    assert_eq!(watcher.next().unwrap(), Event::MoveInto(next_new_file))
}

#[test]
fn test_remove_file() {
    let top_dir = tempfile::tempdir().unwrap();

    let path = top_dir.path().join(random_string(5));
    File::create(&path).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    fs::remove_file(&path).unwrap();
    assert_eq!(watcher.next().unwrap(), Event::DeleteFile(path))
}

#[test]
fn test_remove_dir() {
    let top_dir = tempfile::tempdir().unwrap();

    let dir = top_dir.path().join(random_string(5));
    fs::create_dir(&dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    fs::remove_dir(&dir).unwrap();
    assert_eq!(watcher.next().unwrap(), Event::DeleteDir(dir))
}

#[test]
fn test_remove_top_dir() {
    let top_dir = tempfile::tempdir().unwrap();
    let top_dir = top_dir.path().to_owned();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    fs::remove_dir(&top_dir).unwrap();
    assert_eq!(watcher.next().unwrap(), Event::DeleteTop(top_dir))
}

#[test]
fn test_remove_dir_recursively() {
    let top_dir = tempfile::tempdir().unwrap();

    let dir = top_dir.path().to_owned().join(random_string(5));
    let mut sub_dir = dir.to_owned();
    for _ in 0..3 {
        sub_dir.push(random_string(5));
    }
    fs::create_dir_all(&sub_dir).unwrap();
    let file = sub_dir.join(random_string(5));
    File::create(&file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, false),
    )
    .unwrap();

    fs::remove_dir_all(&dir).unwrap();
    assert_eq!(watcher.next().unwrap(), Event::DeleteFile(file));

    for _ in 0..3 {
        assert_eq!(
            watcher.next().unwrap(),
            Event::DeleteDir(sub_dir.to_owned())
        );
        assert_eq!(watcher.next().unwrap(), Event::Ignored);
        sub_dir.pop();
    }
}