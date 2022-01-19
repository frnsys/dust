use crate::note::Note;
use crate::interval::Interval;

const MAJOR: [usize; 8] = [0, 2, 4, 5, 7, 9, 11, 12];
const MINOR: [usize; 8] = [0, 1, 3, 5, 7, 8, 10, 12];

#[derive(clap::ArgEnum, Debug, Copy, Clone, Eq, PartialEq)]
pub enum Mode {
    Major,
    Minor
}

pub struct Key {
    pub root: Note,
    pub mode: Mode,
}

impl Key {
    /// Compute the interval from the key's root to the
    /// specified scale degree.
    pub fn interval(&self, degree: usize) -> Interval {
        match self.mode {
            Mode::Major => {
                Interval {
                    semitones: MAJOR[degree] as isize
                }
            },
            Mode::Minor => {
                Interval {
                    semitones: MINOR[degree] as isize
                }
            },
        }
    }

    /// Compute the note at the specified scale degree.
    pub fn note(&self, degree: usize) -> Note {
        self.root + self.interval(degree)
    }
}
