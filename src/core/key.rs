use std::fmt;
use super::note::Note;
use super::interval::Interval;

pub const MAJOR: [usize; 7] = [0, 2, 4, 5, 7, 9, 11];
pub const MINOR: [usize; 7] = [0, 1, 3, 5, 7, 8, 10];

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Mode {
    Major,
    Minor
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Mode::Major => "Major",
            Mode::Minor => "Minor"
        };
        write!(f, "{}", name)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Key {
    pub root: Note,
    pub mode: Mode,
}

impl Key {
    /// Compute the interval from the key's root to the
    /// specified scale degree.
    /// Note that by convention scale degrees are 1-indexed;
    /// i.e. degree 1 is the root note of the key,
    /// so we have to subtract 1 to make them 0-indexed.
    pub fn interval(&self, degree: usize) -> Interval {
        let degree = degree - 1;
        match self.mode {
            Mode::Major => {
                Interval {
                    semitones: MAJOR[degree % 7] as isize
                }
            },
            Mode::Minor => {
                Interval {
                    semitones: MINOR[degree % 7] as isize
                }
            },
        }
    }

    /// Compute the note at the specified scale degree.
    pub fn note(&self, degree: usize) -> Note {
        self.root + self.interval(degree)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_interval_major() {
        let key = Key {
            root: "C3".try_into().unwrap(),
            mode: Mode::Major
        };

        let interval = key.interval(1);
        assert_eq!(interval, Interval { semitones: 0 });

        let interval = key.interval(2);
        assert_eq!(interval, Interval { semitones: 2 });

        let interval = key.interval(8);
        assert_eq!(interval, Interval { semitones: 0 });

        let interval = key.interval(9);
        assert_eq!(interval, Interval { semitones: 2 });
    }

    #[test]
    fn test_interval_minor() {
        let key = Key {
            root: "C3".try_into().unwrap(),
            mode: Mode::Minor
        };

        let interval = key.interval(1);
        assert_eq!(interval, Interval { semitones: 0 });

        let interval = key.interval(2);
        assert_eq!(interval, Interval { semitones: 1 });

        let interval = key.interval(8);
        assert_eq!(interval, Interval { semitones: 0 });

        let interval = key.interval(9);
        assert_eq!(interval, Interval { semitones: 1 });
    }
}
