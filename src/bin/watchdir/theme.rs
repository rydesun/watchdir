use termcolor::Color;

use crate::Event;

pub struct Theme {}

impl Theme {
    pub fn head_and_color(&self, event: &Event) -> (&'static str, Color) {
        match event {
            Event::Create(..) => ("Create", Color::Green),
            Event::Delete(..) => ("Delete", Color::Magenta),
            Event::Move(..) => ("Move", Color::Blue),
            Event::MoveAway(..) => ("MoveAway", Color::Blue),
            Event::MoveInto(..) => ("MoveInto", Color::Blue),
            Event::Modify(..) => ("Modify", Color::Yellow),
            Event::Open(..) => ("Open", Color::Cyan),
            Event::OpenTop(..) => ("Open", Color::Cyan),
            Event::Close(..) => ("Close", Color::Cyan),
            Event::CloseTop(..) => ("Close", Color::Cyan),
            Event::Access(..) => ("Access", Color::Cyan),
            Event::AccessTop(..) => ("Access", Color::Cyan),
            Event::Attrib(..) => ("Attrib", Color::Yellow),
            Event::AttribTop(..) => ("Attrib", Color::Yellow),
            Event::MoveTop(..) => ("MoveTop", Color::Red),
            Event::DeleteTop(..) => ("DeleteTop", Color::Red),
            Event::Unmount(..) => ("Unmount", Color::Magenta),
            Event::UnmountTop(..) => ("UnmountTop", Color::Red),
            Event::Unknown | Event::Ignored | Event::Noise => {
                ("Unknown", Color::Red)
            }
        }
    }
}
