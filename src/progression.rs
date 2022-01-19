use std::fmt;
use crate::key::Key;
use crate::chord::Chord;
use rand::seq::SliceRandom;

const NUMERALS: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];

const I: ChordSpec = ChordSpec(0, Quality::Major, vec![]);
const ii: ChordSpec = ChordSpec(1, Quality::Minor, vec![]);
const iii: ChordSpec = ChordSpec(2, Quality::Minor, vec![]);
const IV: ChordSpec = ChordSpec(3, Quality::Major, vec![]);
const V: ChordSpec = ChordSpec(4, Quality::Major, vec![]);
const vi: ChordSpec = ChordSpec(5, Quality::Minor, vec![]);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Quality {
    Major,
    Minor,
    Diminished,
    Augmented
}

// Degree, Quality, Extra notes
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ChordSpec(usize, Quality, Vec<isize>);

impl ChordSpec {
    pub fn new(degree: usize, quality: Quality) -> ChordSpec {
        ChordSpec(degree, quality, vec![])
    }

    /// Add a note by scale degree
    pub fn add(&mut self, degree: usize) -> &mut ChordSpec {
        self.2.push(degree as isize);
        self
    }

    /// Remove a note by scale degree
    pub fn remove(&mut self, degree: usize) -> &mut ChordSpec {
        self.2.push(-(degree as isize));
        self
    }

    /// Resolve the chord spec into actual semitones
    /// for the given key.
    pub fn chord_for_key(&self, key: &Key) -> Chord {
        let root = key.note(self.0);
        let mut intervals = match self.1 {
            Quality::Major => {
                vec![0, 4, 7]
            },
            Quality::Minor => {
                vec![0, 3, 7]
            },
            Quality::Diminished => {
                vec![0, 3, 6]
            },
            Quality::Augmented => {
                vec![0, 4, 8]
            },
        };
        for degree in &self.2 {
            // GTE zero means we add the degree
            if *degree >= 0 {
                let interval = key.interval(*degree as usize);
                intervals.push(interval.semitones);

            // LT zero means we remove the degree
            } else {
                let degree = degree.abs();
                intervals.retain(|s| *s != degree);
            }
        }
        Chord::new(root, intervals)
    }

    /// Diatonic chord progressions
    /// Return a list of candidate chord specs
    /// to follow this one.
    pub fn next(&self) -> Vec<ChordSpec> {
        let chord_name = self.to_string();
        match chord_name.as_ref() {
            "I" => {
                vec![I, ii, iii, vi, IV, V]
            },
            "ii" => {
                vec![V, IV, iii]
            },
            "iii" => {
                vec![vi, I, IV, ii]
            },
            "IV" => {
                vec![I, V, vi, iii]
            },
            "V" => {
                vec![I, IV, vi]
            },
            "vi" => {
                vec![IV, V, I, ii]
            },
            _ => {
                vec![]
            }
        }
    }

    /// Generate a progression of chord specs from this chord spec.
    pub fn gen_progression(&self, bars: usize) -> Vec<ChordSpec> {
        let mut rng = rand::thread_rng();
        let mut progression = vec![self.clone()];
        let mut last = self.clone();
        // TODO clean this up
        for i in 0..bars-1 {
            let cands = last.next();
            let next = cands.choose(&mut rng);
            if next.is_none() {
                break;
            } else {
                last = next.unwrap().clone();
                progression.push(last.clone());
            }
        }
        progression
    }
}

impl fmt::Display for ChordSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut name = NUMERALS[self.0 % 7].to_string();
        let quality = self.1;
        if quality == Quality::Minor || quality == Quality::Diminished {
            name = name.to_lowercase();
        }
        if quality == Quality::Diminished {
            name.push('°');
        } else if quality == Quality::Augmented {
            name.push('+');
        }
        for degree in &self.2 {
            if *degree > 0 {
                name.push_str(&degree.to_string());
            }
        }
        write!(f, "{}", name)
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use crate::key::Mode;
    use crate::note::Note;

    #[test]
    fn test_chord_spec_names() {
        let spec = ChordSpec(0, Quality::Major, vec![]);
        assert_eq!(spec.to_string(), "I".to_string());

        let spec = ChordSpec(2, Quality::Minor, vec![]);
        assert_eq!(spec.to_string(), "iii".to_string());

        let spec = ChordSpec(2, Quality::Diminished, vec![]);
        assert_eq!(spec.to_string(), "iii°".to_string());

        let spec = ChordSpec(2, Quality::Augmented, vec![]);
        assert_eq!(spec.to_string(), "III+".to_string());

        let spec = ChordSpec(2, Quality::Diminished, vec![7]);
        assert_eq!(spec.to_string(), "iii°7".to_string());
    }

    #[test]
    fn test_chord_for_keys() {
        let key = Key {
            root: "C3".try_into().unwrap(),
            mode: Mode::Major,
        };

        let spec = ChordSpec(0, Quality::Major, vec![7]);
        let chord = spec.chord_for_key(&key);
        let notes = chord.notes();
        let expected = vec![Note {
            semitones: 27
        }, Note {
            semitones: 31
        }, Note {
            semitones: 34
        }, Note {
            semitones: 39
        }];
        assert_eq!(notes.len(), expected.len());
        for (a, b) in notes.iter().zip(expected) {
            assert_eq!(*a, b);
        }
    }

    #[test]
    fn test_chord_progression() {
        let bars = 4;
        let spec = ChordSpec(0, Quality::Major, vec![]);
        let progression = spec.gen_progression(bars);
        assert_eq!(progression.len(), bars);
    }
}
