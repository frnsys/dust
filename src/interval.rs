use std::fmt;

const NAMES: [&str; 12] = [
    "Perfect Unison/Octave",
    "Minor Second",
    "Major Second",
    "Minor Third",
    "Major Third", // Diminished fourth
    "Perfect Fourth",
    "Tritone", // Augmented fourth/Diminished fifth
    "Perfect Fifth",
    "Minor Sixth", // Augmented fifth
    "Major Sixth",
    "Minor Seventh",
    "Major Seventh", // Diminished eighth
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_interval_names() {
        let intv = Interval { semitones: 0 };
        assert_eq!(intv.to_string(), "Perfect Unison/Octave".to_string());

        let intv = Interval { semitones: 7 };
        assert_eq!(intv.to_string(), "Perfect Fifth".to_string());

        let intv = Interval { semitones: 12 };
        assert_eq!(intv.to_string(), "Perfect Unison/Octave".to_string());

        let intv = Interval { semitones: -1 };
        assert_eq!(intv.to_string(), "Major Seventh".to_string());
    }
}
