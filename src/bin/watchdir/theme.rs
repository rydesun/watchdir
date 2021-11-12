use termcolor::Color;

use crate::Event;

pub struct Theme {}

impl Theme {
    pub fn head_and_color(&self, event: &Event) -> (&'static str, Color) {
        match event {
            Event::CreateDir(_) => ("Create", Color::Green),
            Event::CreateFile(_) => ("Create", Color::Green),
            Event::DeleteDir(_) => ("Delete", Color::Magenta),
            Event::DeleteFile(_) => ("Delete", Color::Magenta),
            Event::MoveDir(..) => ("Move", Color::Blue),
            Event::MoveFile(..) => ("Move", Color::Blue),
            Event::MoveDirAway(_) => ("MoveAway", Color::Blue),
            Event::MoveFileAway(_) => ("MoveAway", Color::Blue),
            Event::MoveDirInto(_) => ("MoveInto", Color::Blue),
            Event::MoveFileInto(_) => ("MoveInto", Color::Blue),
            Event::Modify(_) => ("Modify", Color::Yellow),
            Event::OpenDir(_) => ("Open", Color::Cyan),
            Event::OpenFile(_) => ("Open", Color::Cyan),
            Event::OpenTop(_) => ("Open", Color::Cyan),
            Event::CloseDir(_) => ("Close", Color::Cyan),
            Event::CloseFile(_) => ("Close", Color::Cyan),
            Event::CloseTop(_) => ("Close", Color::Cyan),
            Event::AccessDir(_) => ("Access", Color::Cyan),
            Event::AccessFile(_) => ("Access", Color::Cyan),
            Event::AccessTop(_) => ("Access", Color::Cyan),
            Event::AttribDir(_) => ("Attrib", Color::Yellow),
            Event::AttribFile(_) => ("Attrib", Color::Yellow),
            Event::AttribTop(_) => ("Attrib", Color::Yellow),
            Event::MoveTop(_) => ("MoveTop", Color::Red),
            Event::DeleteTop(_) => ("DeleteTop", Color::Red),
            Event::Unmount(_) => ("Unmount", Color::Magenta),
            Event::UnmountTop(_) => ("UnmountTop", Color::Red),
            Event::Unknown | Event::Ignored | Event::Noise => {
                ("Unknown", Color::Red)
            }
        }
    }
}
