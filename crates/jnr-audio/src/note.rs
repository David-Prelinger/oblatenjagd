//! Note names (`c4`, `f#3`, `bb2`) as MIDI numbers (`a4` = 69 = 440 Hz).

use std::fmt;

/// A note as its MIDI number. Octaves 0..=8 are accepted (`c0` = 12 … `b8` = 119).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Note(pub u8);

/// MIDI number of `a4`, the 440 Hz reference.
pub const A4: u8 = 69;
const NAMES: [&str; 12] = [
    "c", "c#", "d", "d#", "e", "f", "f#", "g", "g#", "a", "a#", "b",
];

impl Note {
    /// Parses `<letter>[#|b]<octave>`, lowercase, e.g. `c4`, `f#3`, `bb2`.
    pub fn parse(s: &str) -> Option<Note> {
        let mut chars = s.chars();
        let pc: i32 = match chars.next()? {
            'c' => 0,
            'd' => 2,
            'e' => 4,
            'f' => 5,
            'g' => 7,
            'a' => 9,
            'b' => 11,
            _ => return None,
        };
        let rest = chars.as_str();
        let (shift, octave) = match rest.as_bytes().first()? {
            b'#' => (1, &rest[1..]),
            b'b' => (-1, &rest[1..]),
            _ => (0, rest),
        };
        if octave.len() != 1 {
            return None;
        }
        let octave = octave.parse::<i32>().ok().filter(|o| (0..=8).contains(o))?;
        let midi = (octave + 1) * 12 + pc + shift;
        (12..=119).contains(&midi).then_some(Note(midi as u8))
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.0 as usize;
        write!(f, "{}{}", NAMES[n % 12], n / 12 - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_sharps_and_flats() {
        assert_eq!(Note::parse("a4"), Some(Note(69)));
        assert_eq!(Note::parse("c4"), Some(Note(60)));
        assert_eq!(Note::parse("c#4"), Some(Note(61)));
        assert_eq!(Note::parse("db4"), Some(Note(61)));
        assert_eq!(Note::parse("bb2"), Some(Note(46)));
        assert_eq!(Note::parse("c0"), Some(Note(12)));
        assert_eq!(Note::parse("b8"), Some(Note(119)));
        for bad in ["", "h4", "C4", "c", "c9", "c#", "c44", "cb0", "x#1", "c-1"] {
            assert_eq!(Note::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn display_round_trips() {
        for n in 12..=119 {
            let s = Note(n).to_string();
            assert_eq!(Note::parse(&s), Some(Note(n)), "{s}");
        }
    }
}
