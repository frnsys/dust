#[allow(non_upper_case_globals)]

use regex::Regex;
use crate::chord::Chord;
use crate::key::{Key, Mode};
use rand::seq::SliceRandom;
use std::{fmt, str::FromStr};
use thiserror::Error;

const NUMERALS: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];

fn numeral_to_index(numeral: &str) -> Option<usize> {
    NUMERALS.iter().position(|&n| n == numeral.to_uppercase())
}

fn numeral_to_quality(numeral: &str) -> Result<Quality, ChordParseError>{
    if numeral.chars().all(|c| c.is_lowercase()) {
        Ok(Quality::Minor)
    } else if numeral.chars().all(|c| c.is_uppercase()) {
        Ok(Quality::Major)
    } else {
        Err(ChordParseError::InvalidNumeral(numeral.to_string()))
    }
}

fn numeral_to_mode(numeral: &str) -> Result<Mode, ChordParseError>{
    if numeral.chars().all(|c| c.is_lowercase()) {
        Ok(Mode::Minor)
    } else if numeral.chars().all(|c| c.is_uppercase()) {
        Ok(Mode::Major)
    } else {
        Err(ChordParseError::InvalidNumeral(numeral.to_string()))
    }
}


#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Quality {
    Major,
    Minor,
    Diminished,
    Augmented
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ChordSpec {
    degree: usize,
    quality: Quality,
    extras: Vec<isize>,
    bass_degree: usize,
    rel_key: Option<(usize, Mode)>,
}

impl ChordSpec {
    pub fn new(degree: usize, quality: Quality) -> ChordSpec {
        ChordSpec {
            degree,
            quality,
            extras: vec![],
            bass_degree: degree,
            rel_key: None
        }
    }

    /// Add a note by scale degree
    pub fn add(mut self, degree: usize) -> ChordSpec {
        self.extras.push(degree as isize);
        self
    }

    /// Remove a note by scale degree
    pub fn remove(mut self, degree: usize) -> ChordSpec {
        self.extras.push(-(degree as isize));
        self
    }

    /// Set the bass degree
    pub fn bass(mut self, degree: usize) -> ChordSpec {
        self.bass_degree = degree;
        self
    }

    /// Set the relative key, e.g. for secondary dominants
    pub fn key_of(mut self, degree: usize, mode: Mode) -> ChordSpec {
        self.rel_key = Some((degree, mode));
        self
    }

    /// Resolve the chord spec into actual semitones
    /// for the given key.
    pub fn chord_for_key(&self, key: &Key) -> Chord {
        let root = key.note(self.degree);
        let mut intervals = match self.quality {
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
        for degree in &self.extras {
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
        let bass_interval = key.interval(self.bass_degree).semitones;
        let intervals = intervals.iter().map(|intv| if *intv < bass_interval {
            intv + 12
        } else {
            *intv
        }).collect();
        Chord::new(root, intervals)
    }

    /// Chord progressions
    /// Return a list of candidate chord specs
    /// to follow this one.
    pub fn next(&self) -> Vec<ChordSpec> {
        let chord_name = self.to_string();
        let cands = match chord_name.as_ref() {
            // Major
            "I" => vec!["I", "ii", "iii", "vi", "IV", "V"],
            "ii" => vec!["V", "IV", "iii"],
            "iii" => vec!["vi", "I", "IV", "ii"],
            "IV" => vec!["I", "V", "vi", "iii", "ii"],
            "V" => vec!["I", "IV", "vi"],
            "vi" => vec!["IV", "V", "I", "ii"],
            // "vii-/ii"
            _ => vec![]
        };
        cands.into_iter().map(|c| c.try_into().unwrap()).collect()
    }

    /// Generate a progression of chord specs from this chord spec.
    pub fn gen_progression(&self, bars: usize) -> Vec<ChordSpec> {
        let mut rng = rand::thread_rng();
        let mut progression = vec![self.clone()];
        let mut last = self.clone();
        for _ in 0..bars-1 {
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

#[derive(Error, Debug)]
pub enum ChordParseError {
    #[error("Invalid chord `{0}`")]
    InvalidChord(String),

    #[error("Invalid numeral `{0}`")]
    InvalidNumeral(String),

    #[error("Invalid variation `{0}`")]
    InvalidVariation(String),

    #[error("Invalid relative key `{0}`")]
    InvalidRelKey(String),

    #[error("Couldn't parse extra degrees")]
    ParseIntError(#[from] std::num::ParseIntError),
}

/// Try to parse a chord from a string, e.g. "III-7,9".
impl FromStr for ChordSpec {
    type Err = ChordParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = Regex::new(r"^([IV]+|[iv]+)([+-])?([\d,-]*)?(/([IV]+|[iv]+))?(%\d+)?$").unwrap();
        let caps = re.captures(s).ok_or(ChordParseError::InvalidChord(s.to_string()))?;
        let numeral = caps.get(1)
            .ok_or(ChordParseError::InvalidNumeral("(none)".to_string()))?
            .as_str();
        let variation = caps.get(2).and_then(|m| Some(m.as_str()));
        let extras = caps.get(3).and_then(|m| Some(m.as_str()));
        let rel_key = caps.get(5).and_then(|m| Some(m.as_str()));
        let bass_degree = caps.get(6).and_then(|m| Some(m.as_str()));

        let extras: Vec<isize> = if let Some(extras) = extras {
            extras.split(",")
                .filter(|&n| !n.is_empty())
                .map(|n| n.parse()).collect::<Result<Vec<_>, _>>()?
        } else {
            vec![]
        };

        if let Some(degree_0) = numeral_to_index(numeral) {
            let quality = match variation {
                Some(var) => {
                    if var == "-" {
                        Ok(Quality::Diminished)
                    } else if var == "+" {
                        Ok(Quality::Augmented)
                    } else {
                        Err(ChordParseError::InvalidVariation(var.to_string()))
                    }
                }
                None => numeral_to_quality(numeral)
            }?;
            let bass_degree = if let Some(bass) = bass_degree {
                bass[1..].parse()?
            } else {
                degree_0 + 1
            };
            let rel_key = if let Some(rel_key) = rel_key {
                if let Some(degree_0) = numeral_to_index(rel_key) {
                    let mode = numeral_to_mode(rel_key)?;
                    Ok(Some((degree_0 + 1, mode)))
                } else {
                    Err(ChordParseError::InvalidRelKey(rel_key.to_string()))
                }
            } else {
                Ok(None)
            }?;
            Ok(ChordSpec {
                // Convert to 1-indexed degrees
                degree: degree_0 + 1,
                quality,
                extras,
                bass_degree,
                rel_key
            })
        } else {
            Err(ChordParseError::InvalidNumeral(numeral.to_string()))
        }
    }
}

impl TryFrom<&str> for ChordSpec {
    type Error = ChordParseError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Ok(Self::from_str(s)?)
    }
}


impl fmt::Display for ChordSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Convert 1-indexed degree to 0-indexed
        let mut name = NUMERALS[(self.degree - 1) % 7].to_string();
        let quality = self.quality;
        if quality == Quality::Minor || quality == Quality::Diminished {
            name = name.to_lowercase();
        }
        if quality == Quality::Diminished {
            name.push('-');
        } else if quality == Quality::Augmented {
            name.push('+');
        }
        for degree in &self.extras {
            if *degree > 0 {
                name.push_str(&degree.to_string());
            }
        }

        if let Some((degree, mode)) = self.rel_key {
            name.push('/');
            let numeral = NUMERALS[(degree - 1) % 7];
            if mode == Mode::Minor {
                name.push_str(&numeral.to_lowercase());
            } else {
                name.push_str(&numeral);
            }
        }

        if self.degree != self.bass_degree {
            name.push('%');
            name.push_str(&self.bass_degree.to_string());
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
        let spec = ChordSpec::new(1, Quality::Major);
        assert_eq!(spec.to_string(), "I".to_string());

        let spec = ChordSpec::new(3, Quality::Minor);
        assert_eq!(spec.to_string(), "iii".to_string());

        let spec = ChordSpec::new(3, Quality::Diminished);
        assert_eq!(spec.to_string(), "iii-".to_string());

        let spec = ChordSpec::new(3, Quality::Augmented);
        assert_eq!(spec.to_string(), "III+".to_string());

        let spec = ChordSpec::new(3, Quality::Diminished).add(7);
        assert_eq!(spec.to_string(), "iii-7".to_string());

        let spec = ChordSpec::new(3, Quality::Diminished).add(7).bass(5);
        assert_eq!(spec.to_string(), "iii-7%5".to_string());

        let spec = ChordSpec::new(3, Quality::Diminished).add(7).bass(5).key_of(2, Mode::Minor);
        assert_eq!(spec.to_string(), "iii-7/ii%5".to_string());
    }

    #[test]
    fn test_chord_for_keys() {
        let key = Key {
            root: "C3".try_into().unwrap(),
            mode: Mode::Major,
        };

        let spec = ChordSpec::new(1, Quality::Major).add(7);
        let chord = spec.chord_for_key(&key);
        let notes = chord.notes();
        let expected = vec![Note {
            semitones: 27
        }, Note {
            semitones: 31
        }, Note {
            semitones: 34
        }, Note {
            semitones: 38
        }];
        assert_eq!(notes.len(), expected.len());
        for (a, b) in notes.iter().zip(expected) {
            assert_eq!(*a, b);
        }
    }

    #[test]
    fn test_chord_for_keys_inversion() {
        let key = Key {
            root: "C3".try_into().unwrap(),
            mode: Mode::Major,
        };

        let spec = ChordSpec::new(1, Quality::Major).bass(3);
        let chord = spec.chord_for_key(&key);
        let notes = chord.notes();
        let expected = vec![Note {
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
        let spec = ChordSpec::new(1, Quality::Major);
        let progression = spec.gen_progression(bars);
        assert_eq!(progression.len(), bars);
    }

    #[test]
    fn test_parse_chord_spec() {
        let name = "III";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(3, Quality::Major);
        assert_eq!(spec, expected);

        let name = "III7,9";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(3, Quality::Major).add(7).add(9);
        assert_eq!(spec, expected);

        let name = "iii7,9";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(3, Quality::Minor).add(7).add(9);
        assert_eq!(spec, expected);

        let name = "iii-7,9";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(3, Quality::Diminished).add(7).add(9);
        assert_eq!(spec, expected);

        let name = "III+7,9";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(3, Quality::Augmented).add(7).add(9);
        assert_eq!(spec, expected);

        let name = "III+7,9%3";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(3, Quality::Augmented).add(7).add(9).bass(3);
        assert_eq!(spec, expected);

        let name = "V+7,9/ii";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(5, Quality::Augmented).add(7).add(9).key_of(2, Mode::Minor);
        assert_eq!(spec, expected);

        let name = "V+7,9/ii%5";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(5, Quality::Augmented).add(7).add(9).key_of(2, Mode::Minor).bass(5);
        assert_eq!(spec, expected);
    }

}
