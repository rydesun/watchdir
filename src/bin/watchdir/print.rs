use std::{
    collections::HashSet,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use termcolor::{ColorSpec, StandardStream, WriteColor};
use watchdir::Event;

use crate::theme::Theme;

macro_rules! write_color {
    (
        $writer:expr, reset
    ) => {
        $writer.set_color(&ColorSpec::new())
    };

    (
        $writer:expr,
        $( ( $( $fg:expr )? $( ,$bg:expr )? ) )?
        [ $( $effect:ident ),* ]
    ) => {
        $writer.set_color(ColorSpec::new()
            $(
                $(.set_fg(Some($fg)))?
                $(.set_bg(Some($bg)))?
            )?
            $(.$effect(true))*)
    };
}

pub struct Printer {
    stdout: StandardStream,
    theme: Theme,
    top_dir: PathBuf,
    need_prefix: bool,
    timeout_modify: Duration,
    counter: Arc<Mutex<HashSet<PathBuf>>>,
}

impl<'a> Printer {
    pub fn new(
        stdout: StandardStream,
        theme: Theme,
        top_dir: PathBuf,
        need_prefix: bool,
        timeout_modify: u64,
    ) -> Self {
        Self {
            stdout,
            theme,
            top_dir,
            need_prefix,
            timeout_modify: Duration::from_millis(timeout_modify),
            counter: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn print(
        &mut self,
        event: &Event,
        t: time::OffsetDateTime,
    ) -> Result<(), std::io::Error> {
        let (head, color) = self.theme.head_and_color(event);

        write_color!(self.stdout, [set_dimmed])?;
        write!(
            self.stdout,
            "{}  ",
            t.format(&time::macros::format_description!(
                "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond \
                 digits:6]Z"
            ))
            .unwrap(),
        )?;

        match event {
            Event::CreateDir(path)
            | Event::CreateFile(path)
            | Event::DeleteDir(path)
            | Event::DeleteFile(path)
            | Event::MoveDirAway(path)
            | Event::MoveFileAway(path)
            | Event::MoveDirInto(path)
            | Event::MoveFileInto(path)
            | Event::Modify(path)
            | Event::OpenDir(path)
            | Event::OpenFile(path)
            | Event::CloseDir(path)
            | Event::CloseFile(path)
            | Event::AccessDir(path)
            | Event::AccessFile(path)
            | Event::AttribDir(path)
            | Event::AttribFile(path)
            | Event::Unmount(path) => {
                if let Event::Modify(path) = event {
                    if !self.should(path) {
                        return Ok(());
                    }
                }

                let stripped_path = self.strip(path);

                write_color!(self.stdout, (color)[set_bold])?;
                write!(self.stdout, "{:<12}", head)?;

                if self.need_prefix {
                    write_color!(self.stdout, [set_dimmed])?;
                    write!(self.stdout, "{}", self.top_dir.to_string_lossy())?;
                }

                write_color!(self.stdout, (color)[set_bold])?;
                write!(self.stdout, "{}", stripped_path.to_string_lossy())?;

                write_color!(self.stdout, reset)?;
                writeln!(self.stdout)?;
            }
            Event::MoveFile(from_path, to_path)
            | Event::MoveDir(from_path, to_path) => {
                let stripped_from_path = self.strip(from_path);
                let stripped_to_path = self.strip(to_path);

                write_color!(self.stdout, (color)[set_bold])?;
                write!(self.stdout, "{:<12}", head)?;

                if self.need_prefix {
                    write_color!(self.stdout, [set_dimmed])?;
                    write!(self.stdout, "{}", self.top_dir.to_string_lossy())?;
                }

                write_color!(self.stdout, (color)[set_bold])?;
                write!(
                    self.stdout,
                    "{}",
                    stripped_from_path.to_string_lossy()
                )?;

                write_color!(self.stdout, [set_dimmed])?;
                write!(self.stdout, " → ")?;

                if self.need_prefix {
                    write_color!(self.stdout, [set_dimmed])?;
                    write!(self.stdout, "{}", self.top_dir.to_string_lossy())?;
                }

                write_color!(self.stdout, (color)[set_bold])?;
                write!(self.stdout, "{}", stripped_to_path.to_string_lossy())?;

                write_color!(self.stdout, reset)?;
                writeln!(self.stdout)?;
            }
            Event::MoveTop(path)
            | Event::DeleteTop(path)
            | Event::UnmountTop(path)
            | Event::AccessTop(path)
            | Event::AttribTop(path)
            | Event::OpenTop(path)
            | Event::CloseTop(path) => {
                write_color!(self.stdout, (color)[set_bold])?;
                write!(self.stdout, "{:<12}", head)?;

                write_color!(self.stdout, reset)?;
                write!(self.stdout, "{}", path.to_string_lossy())?;

                write_color!(self.stdout, reset)?;
                writeln!(self.stdout)?;
            }
            Event::Unknown | Event::Noise | Event::Ignored => {}
        }
        Ok(())
    }

    pub fn should(&mut self, path: &Path) -> bool {
        if self.timeout_modify.is_zero() {
            true
        } else if self.counter.lock().unwrap().contains(path) {
            false
        } else {
            let timeout = self.timeout_modify;
            let path = path.to_owned();
            let counter = Arc::clone(&self.counter);

            counter.lock().unwrap().insert(path.to_owned());
            tokio::spawn(async move {
                tokio::time::sleep(timeout).await;
                counter.lock().unwrap().remove(&path);
            });
            true
        }
    }

    pub fn strip(&self, path: &'a Path) -> &'a Path {
        path.strip_prefix(&self.top_dir).unwrap()
    }
}
