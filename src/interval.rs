use thiserror::Error;
use std::{fmt, str::FromStr};

const NAMES: [&str; 12] = [
    "P1",
    "m2",
    "M2",
    "m3",
    "M3",
    "P4",
    "d5",
    "P5",
    "m6",
    "M6",
    "m7",
    "M7",
];

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Interval {
    pub semitones: isize
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let idx = self.semitones.rem_euclid(12) as usize;
        let name = NAMES[idx];
        write!(f, "{}", name)
    }
}

/// Generate an interval from an integer.
impl From<isize> for Interval {
    fn from(i: isize) -> Self {
        Interval { semitones: i }
    }
}

#[derive(Error, Debug)]
pub enum IntervalParseError {
    #[error("Invalid interval `{0}`")]
    InvalidInterval(String),
}


impl FromStr for Interval {
    type Err = IntervalParseError;

    /// Parses an interval, e.g. "M3", "m3", "P5", "A5", "d5"
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let semitones = (match s {
            "P1" | "d2" => Ok(0),
            "m2" | "A1" => Ok(1),
            "M2" | "d3" => Ok(2),
            "m3" | "A2" => Ok(3),
            "M3" | "d4" => Ok(4),
            "P4" | "A3" => Ok(5),
            "d5" | "A4" => Ok(6),
            "P5" | "d6" => Ok(7),
            "m6" | "A5" => Ok(8),
            "M6" | "d7" => Ok(9),
            "m7" | "A6" => Ok(10),
            "M7" | "d8" => Ok(11),
            "P8" | "A7" => Ok(12),
            _ => Err(IntervalParseError::InvalidInterval(s.to_string()))
        })?;
        Ok(Interval { semitones })
    }
}

impl TryFrom<&str> for Interval {
    type Error = IntervalParseError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Ok(Self::from_str(s)?)
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_interval_names() {
        let intv = Interval { semitones: 0 };
        assert_eq!(intv.to_string(), "P1".to_string());

        let intv = Interval { semitones: 7 };
        assert_eq!(intv.to_string(), "P5".to_string());

        let intv = Interval { semitones: 12 };
        assert_eq!(intv.to_string(), "P1".to_string());

        let intv = Interval { semitones: -1 };
        assert_eq!(intv.to_string(), "M7".to_string());
    }

    #[test]
    fn test_parse_interval() {
        let intv: Interval = "P1".try_into().unwrap();
        assert_eq!(intv, Interval { semitones: 0 });

        let intv: Interval = "P5".try_into().unwrap();
        assert_eq!(intv, Interval { semitones: 7 });

        let intv: Interval = "P8".try_into().unwrap();
        assert_eq!(intv, Interval { semitones: 12 });

        let intv: Interval = "M3".try_into().unwrap();
        assert_eq!(intv, Interval { semitones: 4 });
    }
}
