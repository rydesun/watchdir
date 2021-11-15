use std::{
    collections::HashSet,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use termcolor::{ColorChoice, ColorSpec, StandardStream, WriteColor};
use watchdir::{Event, FileType};

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
    opts: PrinterOpts,
    stdout: StandardStream,
    counter: Arc<Mutex<HashSet<PathBuf>>>,
    time_offset: Option<time::UtcOffset>,
}

pub struct PrinterOpts {
    pub need_ansi: bool,
    pub color_choice: ColorChoice,
    pub theme: Theme,
    pub top_dir: PathBuf,
    pub need_time: bool,
    pub need_prefix: bool,
    pub timeout_modify: Duration,
}

impl<'a> Printer {
    pub fn new(opts: PrinterOpts) -> Self {
        let color_choice = opts.color_choice.to_owned();
        Self {
            opts,
            stdout: StandardStream::stdout(color_choice),
            counter: Arc::new(Mutex::new(HashSet::new())),
            time_offset: if cfg!(unsound_local_offset) {
                time::UtcOffset::current_local_offset().ok()
            } else {
                None
            },
        }
    }

    pub fn print(
        &mut self,
        event: &Event,
        mut t: time::OffsetDateTime,
    ) -> Result<(), std::io::Error> {
        match event {
            Event::Unknown | Event::Noise | Event::Ignored => return Ok(()),
            Event::Modify(path, _) => {
                if !self.should(path) {
                    return Ok(());
                }
            }
            _ => {}
        }
        let (head, color) = self.opts.theme.head_and_color(event);

        if self.opts.need_ansi {
            self.stdout.write_all(b"\x1b[1000D")?;
        }

        if self.opts.need_time {
            if let Some(offset) = self.time_offset {
                t = t.to_offset(offset);
            }
            write_color!(self.stdout, [set_dimmed])?;
            write!(
                self.stdout,
                "{}",
                t.format(&time::macros::format_description!(
                    "[year]-[month]-[day]T"
                ))
                .unwrap(),
            )?;
            write_color!(self.stdout, [set_bold])?;
            write!(
                self.stdout,
                "{}",
                t.format(&time::macros::format_description!(
                    "[hour]:[minute]:[second]"
                ))
                .unwrap(),
            )?;
            write_color!(self.stdout, [set_dimmed])?;
            write!(
                self.stdout,
                "{}",
                t.format(&time::macros::format_description!(
                    "+[offset_hour][offset_minute]  "
                ))
                .unwrap(),
            )?;
        }

        write_color!(self.stdout, (color)[])?;
        write!(self.stdout, "{:<12}", head)?;

        match event {
            Event::Create(path, file_type)
            | Event::Delete(path, file_type)
            | Event::MoveAway(path, file_type)
            | Event::MoveInto(path, file_type)
            | Event::Modify(path, file_type)
            | Event::Open(path, file_type)
            | Event::Close(path, file_type)
            | Event::Access(path, file_type)
            | Event::Attrib(path, file_type)
            | Event::Unmount(path, file_type) => {
                let mut stripped_path = self.strip(path).to_owned();
                if *file_type == FileType::Dir {
                    stripped_path = stripped_path.join("");
                }

                if self.opts.need_prefix {
                    write_color!(self.stdout, [set_dimmed])?;
                    write!(
                        self.stdout,
                        "{}",
                        self.opts.top_dir.to_string_lossy()
                    )?;
                }

                write_color!(self.stdout, (color)[])?;
                write!(self.stdout, "{}", stripped_path.to_string_lossy())?;
            }
            Event::Move(from_path, to_path, file_type) => {
                let mut stripped_from_path = self.strip(from_path).to_owned();
                let mut stripped_to_path = self.strip(to_path).to_owned();
                if *file_type == FileType::Dir {
                    stripped_from_path = stripped_from_path.join("");
                    stripped_to_path = stripped_to_path.join("");
                }

                if self.opts.need_prefix {
                    write_color!(self.stdout, [set_dimmed])?;
                    write!(
                        self.stdout,
                        "{}",
                        self.opts.top_dir.to_string_lossy()
                    )?;
                }

                write_color!(self.stdout, (color)[])?;
                write!(
                    self.stdout,
                    "{}",
                    stripped_from_path.to_string_lossy()
                )?;

                write_color!(self.stdout, [set_dimmed])?;
                write!(self.stdout, " → ")?;

                if self.opts.need_prefix {
                    write_color!(self.stdout, [set_dimmed])?;
                    write!(
                        self.stdout,
                        "{}",
                        self.opts.top_dir.to_string_lossy()
                    )?;
                }

                write_color!(self.stdout, (color)[])?;
                write!(self.stdout, "{}", stripped_to_path.to_string_lossy())?;
            }
            Event::MoveTop(path)
            | Event::DeleteTop(path)
            | Event::UnmountTop(path)
            | Event::AccessTop(path)
            | Event::AttribTop(path)
            | Event::OpenTop(path)
            | Event::CloseTop(path) => {
                write_color!(self.stdout, [set_dimmed])?;
                write!(self.stdout, "{}", path.to_string_lossy())?;
            }
            _ => {}
        }

        write_color!(self.stdout, reset)?;
        writeln!(self.stdout)?;
        Ok(())
    }

    pub fn should(&mut self, path: &Path) -> bool {
        if self.opts.timeout_modify.is_zero() {
            true
        } else if self.counter.lock().unwrap().contains(path) {
            false
        } else {
            let timeout = self.opts.timeout_modify;
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
        path.strip_prefix(&self.opts.top_dir).unwrap()
    }
}
