use termcolor::Color;

use crate::Event;

pub struct Theme {}

impl Theme {
    pub fn head_and_color(&self, event: &Event) -> (&'static str, Color) {
        match event {
            Event::Create(_) => ("Create", Color::Green),
            Event::DeleteDir(_) => ("Delete", Color::Magenta),
            Event::DeleteFile(_) => ("Delete", Color::Magenta),
            Event::MoveDir(..) => ("Move", Color::Blue),
            Event::MoveFile(..) => ("Move", Color::Blue),
            Event::MoveAwayDir(_) => ("MoveAway", Color::Blue),
            Event::MoveAwayFile(_) => ("MoveAway", Color::Blue),
            Event::MoveInto(_) => ("MoveInto", Color::Blue),
            Event::Modify(_) => ("Modify", Color::Yellow),
            Event::Open(_) => ("Open", Color::Cyan),
            Event::OpenTop(_) => ("Open", Color::Cyan),
            Event::Close(_) => ("Close", Color::Cyan),
            Event::CloseTop(_) => ("Close", Color::Cyan),
            Event::Access(_) => ("Access", Color::Cyan),
            Event::AccessTop(_) => ("Access", Color::Cyan),
            Event::Attrib(_) => ("Attrib", Color::Yellow),
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
