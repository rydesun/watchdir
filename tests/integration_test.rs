use std::{
    fs::{self, File},
    path::PathBuf,
};

use futures::{pin_mut, StreamExt};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use watchdir::*;

fn random_string(len: usize) -> String {
    thread_rng().sample_iter(&Alphanumeric).take(len).map(char::from).collect()
}

#[tokio::test]
async fn test_create_file() {
    let top_dir = tempfile::tempdir().unwrap();
    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let path = top_dir.path().join(random_string(5));
    File::create(&path).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Create(path, FileType::File)
    )
}

#[tokio::test]
async fn test_create_in_created_subdir() {
    let top_dir = tempfile::tempdir().unwrap();
    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let dir = top_dir.path().join(random_string(5));
    fs::create_dir(&dir).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Create(dir.to_owned(), FileType::Dir)
    );

    let path = dir.join(random_string(5));
    File::create(&path).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Create(path, FileType::File)
    )
}

#[tokio::test]
async fn test_create_in_recur_created_subdir() {
    let top_dir = tempfile::tempdir().unwrap();
    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let recur_depth = 3;
    let mut dir = top_dir.path().to_owned();
    let mut dirs: Vec<PathBuf> = Vec::<PathBuf>::new();
    for _ in 0..recur_depth {
        dir = dir.join(random_string(5));
        dirs.push(dir.to_owned());
    }
    fs::create_dir_all(&dir).unwrap();
    for d in dirs.iter().take(recur_depth) {
        assert_eq!(
            stream.next().await.unwrap().0,
            Event::Create(d.to_owned(), FileType::Dir)
        );
    }

    let path = dir.join(random_string(5));
    File::create(&path).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Create(path, FileType::File)
    )
}

#[tokio::test]
async fn test_move_dir() {
    let top_dir = tempfile::tempdir().unwrap();
    let old_dir = top_dir.path().join(random_string(5));
    fs::create_dir(&old_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let new_dir = top_dir.path().join(random_string(5));
    fs::rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Move(old_dir, new_dir, FileType::Dir)
    )
}

#[tokio::test]
async fn test_move_long_name_dir() {
    let top_dir = tempfile::tempdir().unwrap();
    let old_dir = top_dir.path().join(random_string(180));
    fs::create_dir(&old_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let new_dir = top_dir.path().join(random_string(180));
    fs::rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Move(old_dir, new_dir, FileType::Dir)
    )
}

#[tokio::test]
async fn test_move_top_dir() {
    let top_dir = tempfile::tempdir().unwrap();
    let top_dir = top_dir.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let new_top_dir = temp_dir.path().join(random_string(5));

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    fs::rename(&top_dir, new_top_dir).unwrap();

    assert_eq!(stream.next().await.unwrap().0, Event::MoveTop(top_dir))
}

#[tokio::test]
async fn test_create_in_moved_subdir() {
    let top_dir = tempfile::tempdir().unwrap();

    let old_dir = top_dir.path().join(random_string(5));

    let mut sub_dirs = PathBuf::new();
    for _ in 0..3 {
        sub_dirs.push(PathBuf::from(random_string(5)));
    }
    fs::create_dir_all(&old_dir.join(sub_dirs.to_owned())).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let new_dir = top_dir.path().join(random_string(5));

    fs::rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Move(old_dir, new_dir.to_owned(), FileType::Dir)
    );

    let new_file = new_dir.join(sub_dirs).join(random_string(5));
    File::create(&new_file).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Create(new_file, FileType::File)
    )
}

#[tokio::test]
async fn test_create_in_moved_dir_in_subdir() {
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
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let new_dir = sub_dirs.to_owned().join(random_string(5));
    fs::rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Move(old_dir, new_dir.to_owned(), FileType::Dir)
    );

    let new_file = new_dir.join(random_string(5));
    File::create(&new_file).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Create(new_file, FileType::File)
    )
}

#[tokio::test]
async fn test_move_file() {
    let top_dir = tempfile::tempdir().unwrap();
    let old_file = top_dir.path().join(random_string(5));
    File::create(&old_file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let new_file = top_dir.path().join(random_string(5));
    fs::rename(old_file.to_owned(), new_file.to_owned()).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Move(old_file, new_file, FileType::File)
    )
}

#[tokio::test]
async fn test_dir_move_away() {
    let top_dir = tempfile::tempdir().unwrap();
    let unwatched_dir = tempfile::tempdir().unwrap();
    let old_dir = top_dir.path().join(random_string(5));
    fs::create_dir(&old_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let new_dir = unwatched_dir.path().join(random_string(5));
    fs::rename(old_dir.to_owned(), new_dir.to_owned()).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::MoveAway(old_dir, FileType::Dir)
    );

    let unwatched_file = new_dir.join(random_string(5));
    File::create(&unwatched_file).unwrap();
    assert_eq!(stream.next().await.unwrap().0, Event::Ignored);
}

#[tokio::test]
async fn test_file_move_away() {
    let top_dir = tempfile::tempdir().unwrap();
    let unwatched_dir = tempfile::tempdir().unwrap();
    let old_file = top_dir.path().join(random_string(5));
    File::create(&old_file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let new_file = unwatched_dir.path().join(random_string(5));
    fs::rename(old_file.to_owned(), new_file).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::MoveAway(old_file, FileType::File)
    );
}

#[tokio::test]
async fn test_dir_move_into() {
    let top_dir = tempfile::tempdir().unwrap();
    let unwatched_dir = tempfile::tempdir().unwrap();
    let old_dir = unwatched_dir.path().join(random_string(5));
    fs::create_dir(&old_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let new_dir = top_dir.path().join(random_string(5));
    fs::rename(old_dir, new_dir.to_owned()).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::MoveInto(new_dir.to_owned(), FileType::Dir)
    );

    let new_file = new_dir.join(random_string(5));
    File::create(&new_file).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Create(new_file, FileType::File)
    );
}

#[tokio::test]
async fn test_file_move_into() {
    let top_dir = tempfile::tempdir().unwrap();
    let unwatched_dir = tempfile::tempdir().unwrap();
    let old_file = unwatched_dir.path().join(random_string(5));
    File::create(&old_file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let new_file = top_dir.path().join(random_string(5));
    fs::rename(old_file, new_file.to_owned()).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::MoveInto(new_file, FileType::File)
    );
}

#[tokio::test]
async fn test_file_move_away_and_move_into() {
    let top_dir = tempfile::tempdir().unwrap();
    let unwatched_dir = tempfile::tempdir().unwrap();

    let old_file = top_dir.path().join(random_string(5));
    File::create(&old_file).unwrap();

    let next_file_name = random_string(5);
    let next_old_file = unwatched_dir.path().join(next_file_name.to_owned());
    File::create(&next_old_file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let new_file = unwatched_dir.path().join(random_string(5));
    fs::rename(old_file.to_owned(), new_file).unwrap();
    let next_new_file = top_dir.path().join(next_file_name);
    fs::rename(next_old_file, next_new_file.to_owned()).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::MoveAway(old_file, FileType::File)
    );
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::MoveInto(next_new_file, FileType::File)
    )
}

#[tokio::test]
async fn test_remove_file() {
    let top_dir = tempfile::tempdir().unwrap();

    let path = top_dir.path().join(random_string(5));
    File::create(&path).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    fs::remove_file(&path).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Delete(path, FileType::File)
    )
}

#[tokio::test]
async fn test_remove_dir() {
    let top_dir = tempfile::tempdir().unwrap();

    let dir = top_dir.path().join(random_string(5));
    fs::create_dir(&dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    fs::remove_dir(&dir).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Delete(dir, FileType::Dir)
    )
}

#[tokio::test]
async fn test_remove_top_dir() {
    let top_dir = tempfile::tempdir().unwrap();
    let top_dir = top_dir.path().to_owned();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    fs::remove_dir(&top_dir).unwrap();
    assert_eq!(stream.next().await.unwrap().0, Event::DeleteTop(top_dir))
}

#[tokio::test]
async fn test_remove_dir_recursively() {
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
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    fs::remove_dir_all(&dir).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Delete(file, FileType::File)
    );

    for _ in 0..3 {
        assert_eq!(
            stream.next().await.unwrap().0,
            Event::Delete(sub_dir.to_owned(), FileType::Dir)
        );
        assert_eq!(stream.next().await.unwrap().0, Event::Ignored);
        sub_dir.pop();
    }
}

#[tokio::test]
async fn test_modify_file() {
    let top_dir = tempfile::tempdir().unwrap();
    let file = top_dir.path().join(random_string(5));
    File::create(&file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::from([ExtraEvent::Modify])),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    fs::write(&file, "test").unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Modify(file, FileType::File)
    );
}

#[tokio::test]
async fn test_open_file() {
    let top_dir = tempfile::tempdir().unwrap();
    let file = top_dir.path().join(random_string(5));
    File::create(&file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::from([ExtraEvent::Open])),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    fs::File::open(&file).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::OpenTop(top_dir.path().to_owned())
    );
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Open(file, FileType::File)
    );
}

#[tokio::test]
async fn test_open_dir() {
    let top_dir = tempfile::tempdir().unwrap();
    let sub_dir = top_dir.path().join(random_string(5));
    fs::create_dir(&sub_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::from([ExtraEvent::Open])),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    fs::File::open(&sub_dir).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::OpenTop(top_dir.path().to_owned())
    );
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Open(sub_dir, FileType::Dir)
    );
}

#[tokio::test]
async fn test_close_file() {
    let top_dir = tempfile::tempdir().unwrap();
    let file = top_dir.path().join(random_string(5));
    File::create(&file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::from([ExtraEvent::Close])),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    fs::File::open(&file).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::CloseTop(top_dir.path().to_owned())
    );
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Close(file, FileType::File)
    );
}

#[tokio::test]
async fn test_close_dir() {
    let top_dir = tempfile::tempdir().unwrap();
    let sub_dir = top_dir.path().join(random_string(5));
    fs::create_dir(&sub_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::from([ExtraEvent::Close])),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Close(sub_dir, FileType::Dir)
    );
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::CloseTop(top_dir.path().to_owned())
    );
}

#[tokio::test]
async fn test_access_file() {
    let top_dir = tempfile::tempdir().unwrap();
    let file = top_dir.path().join(random_string(5));
    fs::write(&file, "test").unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::from([ExtraEvent::Access])),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    fs::read(&file).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::AccessTop(top_dir.path().to_owned())
    );
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Access(file, FileType::File)
    );
}

#[tokio::test]
async fn test_access_dir() {
    let top_dir = tempfile::tempdir().unwrap();
    let sub_dir = top_dir.path().join(random_string(5));
    fs::create_dir(&sub_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::from([ExtraEvent::Access])),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::AccessTop(top_dir.path().to_owned())
    );
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Access(sub_dir, FileType::Dir)
    );
}

#[tokio::test]
async fn test_attrib_file() {
    let top_dir = tempfile::tempdir().unwrap();
    let file = top_dir.path().join(random_string(5));
    File::create(&file).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::from([ExtraEvent::Attrib])),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let mut perms = fs::metadata(&file).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&file, perms).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Attrib(file, FileType::File)
    );
}

#[tokio::test]
async fn test_attrib_dir() {
    let top_dir = tempfile::tempdir().unwrap();
    let sub_dir = top_dir.path().join(random_string(5));
    fs::create_dir(&sub_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::from([ExtraEvent::Attrib])),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let mut perms = fs::metadata(&sub_dir).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&sub_dir, perms).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Attrib(sub_dir, FileType::Dir)
    );

    let mut perms = fs::metadata(&top_dir).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&top_dir, perms).unwrap();

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::AttribTop(top_dir.path().to_owned())
    );
}

#[tokio::test]
async fn test_include_hidden_dir() {
    let tempdir = tempfile::tempdir().unwrap();
    let dotdir = tempdir.as_ref().join(".dotdir");
    fs::create_dir(&dotdir).unwrap();

    let mut watcher = Watcher::new(
        tempdir.as_ref(),
        WatcherOpts::new(Dotdir::Include, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let file = dotdir.join(random_string(5));
    File::create(&file).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Create(file, FileType::File)
    );
}

#[tokio::test]
async fn test_exclude_hidden_dir() {
    let tempdir = tempfile::tempdir().unwrap();
    let dotdir = tempdir.as_ref().join(".dotdir");
    fs::create_dir(&dotdir).unwrap();

    let mut watcher = Watcher::new(
        tempdir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();

    let file = dotdir.join(random_string(5));
    File::create(&file).unwrap();
    assert!(!watcher.has_next_event());
}

#[tokio::test]
async fn test_exclude_new_hidden_dir() {
    let tempdir = tempfile::tempdir().unwrap();

    let mut watcher = Watcher::new(
        tempdir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();

    let dotdir = tempdir.as_ref().join(".dotdir");
    fs::create_dir(&dotdir).unwrap();
    {
        let stream = watcher.stream();
        pin_mut!(stream);
        assert_eq!(
            stream.next().await.unwrap().0,
            Event::Create(dotdir.to_owned(), FileType::Dir)
        );
    }

    let file = dotdir.join(random_string(5));
    File::create(&file).unwrap();
    assert!(!watcher.has_next_event());
}

#[tokio::test]
async fn test_unwatch_moved_hidden_dir() {
    let tempdir = tempfile::tempdir().unwrap();

    let dir = tempdir.as_ref().join("dir");
    fs::create_dir(&dir).unwrap();

    let mut watcher = Watcher::new(
        tempdir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();

    let dotdir = tempdir.as_ref().join(".dotdir");
    fs::rename(&dir, &dotdir).unwrap();
    {
        let stream = watcher.stream();
        pin_mut!(stream);

        assert_eq!(
            stream.next().await.unwrap().0,
            Event::Move(dir.to_owned(), dotdir.to_owned(), FileType::Dir)
        );
        let file = dotdir.join(random_string(5));
        File::create(&file).unwrap();
        assert_eq!(stream.next().await.unwrap().0, Event::Ignored);
    }
    assert!(!watcher.has_next_event());
}

#[tokio::test]
async fn test_rewatch_moved_hidden_dir() {
    let tempdir = tempfile::tempdir().unwrap();

    let dotdir = tempdir.as_ref().join(".dotdir");
    fs::create_dir(&dotdir).unwrap();

    let mut watcher = Watcher::new(
        tempdir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();

    let dir = tempdir.as_ref().join("dir");
    fs::rename(&dotdir, &dir).unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Move(dotdir.to_owned(), dir.to_owned(), FileType::Dir)
    );
    let file = dir.join(random_string(5));
    File::create(&file).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Create(file, FileType::File)
    );
}

#[tokio::test]
async fn test_must_include_hidden_top_dir() {
    let tempdir = tempfile::tempdir().unwrap();
    let top_dir = tempdir.as_ref().join(".dotdir");
    fs::create_dir(&top_dir).unwrap();

    let mut watcher = Watcher::new(
        top_dir.as_ref(),
        WatcherOpts::new(Dotdir::Exclude, Vec::new()),
    )
    .unwrap();
    let stream = watcher.stream();
    pin_mut!(stream);

    let file = top_dir.join(random_string(5));
    File::create(&file).unwrap();
    assert_eq!(
        stream.next().await.unwrap().0,
        Event::Create(file, FileType::File)
    );
}
