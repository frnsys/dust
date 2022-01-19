use crate::interval::Interval;
use thiserror::Error;
use itertools::Itertools;
use std::{fmt, str::FromStr};
use std::ops::{Add, Sub};

const NAMES: [&str; 12] = ["A", "Bb", "B", "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab"];

/// 0 semitones = "A0".
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Note {
    pub semitones: isize,
}

#[derive(Error, Debug)]
pub enum NoteParseError {
    #[error("Invalid note name `{0}`")]
    InvalidName(String),

    #[error("Couldn't parse octave")]
    ParseIntError(#[from] std::num::ParseIntError),
}

/// Try to parse a note from a string, e.g. "C3".
impl FromStr for Note {
    type Err = NoteParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        let name = chars.take_while_ref(|c| c.is_alphabetic())
            .collect::<String>();
        let octave = chars.collect::<String>();

        if let Some(offset) = NAMES.iter().position(|&n| n == name) {
            let offset = offset as isize;
            let mut octave = octave.parse::<isize>()?;
            octave -= (offset+9)/12;
            let semitones = (octave * 12) + (offset % 12);
            Ok(Note { semitones })
        } else {
            Err(NoteParseError::InvalidName(name.to_string()))
        }
    }
}

impl TryFrom<&str> for Note {
    type Error = NoteParseError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Ok(Self::from_str(s)?)
    }
}

impl TryFrom<String> for Note {
    type Error = NoteParseError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(Self::from_str(&s)?)
    }
}


impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let idx = self.semitones.rem_euclid(12) as usize;
        let name = NAMES[idx];
        let octave = (self.semitones + 9) / 12;
        write!(f, "{}{}", name, octave)
    }
}

/// Add an interval to this note.
impl Add<Interval> for Note {
    type Output = Self;

    fn add(self, intv: Interval) -> Self {
        Self {
            semitones: self.semitones + intv.semitones
        }
    }
}

/// Subtract an interval to this note.
impl Sub<Interval> for Note {
    type Output = Self;

    fn sub(self, intv: Interval) -> Self {
        Self {
            semitones: self.semitones - intv.semitones
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_note_names() {
        let note = Note { semitones: 0 };
        assert_eq!(note.to_string(), "A0".to_string());

        let note = Note { semitones: 1 };
        assert_eq!(note.to_string(), "Bb0".to_string());

        let note = Note { semitones: 2 };
        assert_eq!(note.to_string(), "B0".to_string());

        let note = Note { semitones: 3 };
        assert_eq!(note.to_string(), "C1".to_string());

        let note = Note { semitones: 27 };
        assert_eq!(note.to_string(), "C3".to_string());

        let note = Note { semitones: -1 };
        assert_eq!(note.to_string(), "Ab0".to_string());
    }

    #[test]
    fn test_parse_note() {
        let name = "A0";
        let note: Note = name.try_into().unwrap();
        assert_eq!(note.semitones, 0);

        let name = "Bb0";
        let note: Note = name.try_into().unwrap();
        assert_eq!(note.semitones, 1);

        let name = "B0";
        let note: Note = name.try_into().unwrap();
        assert_eq!(note.semitones, 2);

        let name = "C1";
        let note: Note = name.try_into().unwrap();
        assert_eq!(note.semitones, 3);

        let name = "C3";
        let note: Note = name.try_into().unwrap();
        assert_eq!(note.semitones, 27);

        let name = "Ab0";
        let note: Note = name.try_into().unwrap();
        assert_eq!(note.semitones, -1);
    }

    #[test]
    fn test_interval_math() {
        let note = Note { semitones: 10 };
        let intv = Interval { semitones: 2 };

        let new_note = note + intv;
        assert_eq!(new_note.semitones, 12);

        let new_note = note - intv;
        assert_eq!(new_note.semitones, 8);
    }
}
