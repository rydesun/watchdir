use std::str::FromStr;

use serde::{de, Deserialize, Deserializer};

use crate::Event;

struct Color(termcolor::Color);

#[derive(Deserialize)]
#[serde(default)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct Theme {
    create: Color,
    delete: Color,
    r#move: Color,
    move_away: Color,
    move_into: Color,
    modify: Color,
    open: Color,
    close: Color,
    access: Color,
    attrib: Color,
    umount: Color,
}

impl Theme {
    pub fn head_and_color(
        &self,
        event: &Event,
    ) -> (&'static str, termcolor::Color) {
        match event {
            Event::Create(..) => ("Create", self.create.0),
            Event::Delete(..) => ("Delete", self.delete.0),
            Event::Move(..) => ("Move", self.r#move.0),
            Event::MoveAway(..) => ("MoveAway", self.move_away.0),
            Event::MoveInto(..) => ("MoveInto", self.move_into.0),
            Event::Modify(..) => ("Modify", self.modify.0),
            Event::Open(..) => ("Open", self.open.0),
            Event::OpenTop(..) => ("Open", self.open.0),
            Event::Close(..) => ("Close", self.close.0),
            Event::CloseTop(..) => ("Close", self.close.0),
            Event::Access(..) => ("Access", self.access.0),
            Event::AccessTop(..) => ("Access", self.access.0),
            Event::Attrib(..) => ("Attrib", self.attrib.0),
            Event::AttribTop(..) => ("Attrib", self.attrib.0),
            Event::MoveTop(..) => ("MoveTop", self.r#move.0),
            Event::DeleteTop(..) => ("DeleteTop", self.delete.0),
            Event::Unmount(..) => ("Unmount", self.umount.0),
            Event::UnmountTop(..) => ("UnmountTop", self.umount.0),
            Event::Unknown | Event::Ignored | Event::Noise => {
                unimplemented!();
            }
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            create: Color(termcolor::Color::Green),
            delete: Color(termcolor::Color::Magenta),
            r#move: Color(termcolor::Color::Blue),
            move_away: Color(termcolor::Color::Blue),
            move_into: Color(termcolor::Color::Blue),
            modify: Color(termcolor::Color::Yellow),
            open: Color(termcolor::Color::Cyan),
            close: Color(termcolor::Color::Cyan),
            access: Color(termcolor::Color::Cyan),
            attrib: Color(termcolor::Color::Yellow),
            umount: Color(termcolor::Color::Magenta),
        }
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map(Color).map_err(de::Error::custom)
    }
}
