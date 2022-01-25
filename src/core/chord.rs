use regex::Regex;
use thiserror::Error;
use std::{fmt, str::FromStr};
use super::note::Note;
use super::interval::Interval;
use super::key::{Key, Mode, MAJOR, MINOR};

const NUMERALS: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];

fn numeral_to_index(numeral: &str) -> Option<usize> {
    NUMERALS.iter().position(|&n| n == numeral.to_uppercase())
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

#[derive(Debug, Clone, Eq, PartialEq)]
struct Extension {
    degree: usize,
    adj: isize
}

impl Extension {
    pub fn to_interval(&self, mode: &Mode) -> isize {
        // Convert from 1-indexed to 0-indexed degrees
        let degree_0 = self.degree - 1;
        let octaves = (degree_0/8 * 12) as isize;
        (match mode {
            Mode::Major => MAJOR[degree_0 % 7],
            Mode::Minor => MINOR[degree_0 % 7]
        } as isize) + octaves + self.adj
    }
}

impl FromStr for Extension {
    type Err = ChordParseError;

    /// Parses a chord extension, e.g. "7", "b7", "#9", "bb7"
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = Regex::new(r"^([b#]*)(\d+)$").unwrap();
        let caps = re.captures(s).ok_or(ChordParseError::InvalidExtension(s.to_string()))?;
        let adjustments = caps.get(1).and_then(|m| Some(m.as_str())).unwrap_or_default();
        let degree = caps.get(2)
            .ok_or(ChordParseError::InvalidExtension("(none)".to_string()))?
            .as_str().parse::<usize>()?;
        let adj = adjustments.matches('#').count() as isize - adjustments.matches('b').count() as isize;
        Ok(Extension { degree, adj })

    }
}

impl TryFrom<&str> for Extension {
    type Error = ChordParseError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Ok(Self::from_str(s)?)
    }
}

impl fmt::Display for Extension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = "".to_string();
        let count = self.adj.abs() as usize;
        if self.adj < 0 {
            s.push_str(&std::iter::repeat("b").take(count).collect::<String>());
        } else if self.adj > 0 {
            s.push_str(&std::iter::repeat("#").take(count).collect::<String>());
        }
        write!(f, "{}{}", s, self.degree)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Triad {
    Mode,
    Diminished,
    Augmented,
    Sus2,
    Sus4,
    Power,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ChordSpec {
    degree: usize,
    adj: isize,
    mode: Mode,
    triad: Triad,
    extensions: Vec<Extension>,
    bass_degree: Option<Extension>,
    rel_key: Option<(usize, Mode)>,
}

impl ChordSpec {
    pub fn new(degree: usize, mode: Mode) -> ChordSpec {
        ChordSpec {
            degree,
            mode,
            adj: 0,
            triad: Triad::Mode,
            extensions: vec![],
            bass_degree: None,
            rel_key: None
        }
    }

    /// Set the triad type for this chord
    pub fn triad(mut self, triad: Triad) -> ChordSpec {
        self.triad = triad;
        self
    }

    /// Add a note by scale degree and step adjustment
    ///
    /// Examples:
    ///
    /// ```
    /// // Adds a 7
    /// cs.add(7, 0)
    ///
    /// // Adds a "b7"
    /// cs.add(7, -1)
    ///
    /// // Adds a "#7"
    /// cs.add(7, 1)
    /// ```
    pub fn add(mut self, degree: usize, adj: isize) -> ChordSpec {
        self.extensions.push(Extension { degree, adj });
        self
    }

    /// Set the bass degree
    pub fn bass(mut self, degree: usize, adj: isize) -> ChordSpec {
        self.bass_degree = Some(Extension { degree, adj });
        self
    }

    /// Set the relative key, e.g. for secondary dominants
    pub fn key_of(mut self, degree: usize, mode: Mode) -> ChordSpec {
        self.rel_key = Some((degree, mode));
        self
    }

    /// Sets an semitone adjustment, for chromatic roots
    pub fn adj(mut self, adj: isize) -> ChordSpec {
        self.adj = adj;
        self
    }

    /// Shift by a number of octaves
    pub fn shift(mut self, octaves: isize) -> ChordSpec {
        self.adj += octaves * 12;
        self
    }

    pub fn intervals(&self) -> Vec<isize> {
        let offset = match self.rel_key {
            None => 0,
            Some((degree, _)) => {
                match self.mode {
                    Mode::Major => MAJOR[degree - 1 % 7],
                    Mode::Minor => MINOR[degree - 1 % 7]
                }
            }
        } as isize + self.adj;

        let mode = match self.rel_key {
            None => self.mode,
            Some((_, mode)) => mode
        };

        let mut intervals = match self.triad {
            Triad::Mode => {
                match self.mode {
                    Mode::Major => {
                        vec![0, 4, 7]
                    }
                    Mode::Minor => {
                        vec![0, 3, 7]
                    }
                }
            }
            Triad::Diminished => {
                vec![0, 3, 6]
            }
            Triad::Augmented => {
                vec![0, 4, 8]
            }
            Triad::Sus2 => {
                vec![0, 2, 7]
            }
            Triad::Sus4 => {
                vec![0, 5, 7]
            }
            Triad::Power => {
                vec![0, 7]
            }
        };

        for ext in &self.extensions {
            intervals.push(ext.to_interval(&mode));
        }

        let intervals = if let Some(bass_degree) = &self.bass_degree {
            let bass_interval = bass_degree.to_interval(&mode);
            intervals.iter().map(|intv| if *intv < bass_interval {
                intv + 12
            } else {
                *intv
            }).collect()
        } else {
            intervals
        };
        intervals.iter().map(|intv| offset + *intv).collect()
    }

    /// Resolve the chord spec into actual semitones
    /// for the given key.
    pub fn chord_for_key(&self, key: &Key) -> Chord {
        let root = key.note(self.degree);
        Chord::new(root, self.intervals())
    }
}

#[derive(Error, Debug)]
pub enum ChordParseError {
    #[error("Invalid chord `{0}`")]
    InvalidChord(String),

    #[error("Invalid numeral `{0}`")]
    InvalidNumeral(String),

    #[error("Invalid triad symbol `{0}`")]
    InvalidTriadSymbol(String),

    #[error("Invalid extension `{0}`")]
    InvalidExtension(String),

    #[error("Invalid relative key `{0}`")]
    InvalidRelKey(String),

    #[error("Couldn't parse extension")]
    ParseIntError(#[from] std::num::ParseIntError),
}

/// Try to parse a chord from a string, e.g. "III-7,9".
impl FromStr for ChordSpec {
    type Err = ChordParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = Regex::new(r"^([IV]+|[iv]+)([b#])*([+-^_5])?(:([b#]?\d+,?)*)?(/([b#]?\d+))?(>\d+)?(<\d+)?(~([IV]+|[iv]+))?$").unwrap();
        let caps = re.captures(s).ok_or(ChordParseError::InvalidChord(s.to_string()))?;
        let numeral = caps.get(1)
            .ok_or(ChordParseError::InvalidNumeral("(none)".to_string()))?
            .as_str();
        let adj = caps.get(2).and_then(|m| Some(m.as_str()));
        let triad = caps.get(3).and_then(|m| Some(m.as_str()));
        let exts = caps.get(4).and_then(|m| Some(m.as_str()));
        let bass_degree = caps.get(6).and_then(|m| Some(m.as_str()));
        let shift_up = caps.get(8).and_then(|m| Some(m.as_str()));
        let shift_down = caps.get(9).and_then(|m| Some(m.as_str()));
        let rel_key = caps.get(11).and_then(|m| Some(m.as_str()));

        let mode = numeral_to_mode(numeral)?;
        let mut adj = match adj {
            Some(adj) => adj.matches('#').count() as isize - adj.matches('b').count() as isize,
            None => 0
        };

        if let Some(shift_up) = shift_up {
            let octaves = shift_up[1..].parse::<isize>()?;
            adj += octaves * 12;
        }
        if let Some(shift_down) = shift_down {
            let octaves = shift_down[1..].parse::<isize>()?;
            adj -= octaves * 12;
        }

        let triad = match triad {
            Some(triad) => {
                if triad == "-" {
                    Ok(Triad::Diminished)
                } else if triad == "+" {
                    Ok(Triad::Augmented)
                } else if triad == "_" {
                    Ok(Triad::Sus2)
                } else if triad == "^" {
                    Ok(Triad::Sus4)
                } else if triad == "5" {
                    Ok(Triad::Power)
                } else {
                    Err(ChordParseError::InvalidTriadSymbol(triad.to_string()))
                }
            }
            None => Ok(Triad::Mode)
        }?;

        let exts: Vec<Extension> = if let Some(exts) = exts {
            exts[1..].split(",")
                .filter(|&n| !n.is_empty())
                .map(|n| n.try_into()).collect::<Result<Vec<_>, _>>()?
        } else {
            vec![]
        };

        if let Some(degree_0) = numeral_to_index(numeral) {
            let bass_degree = if let Some(bass) = bass_degree {
                Some(bass[1..].try_into()?)
            } else {
                None
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
                adj,
                triad,
                mode,
                extensions: exts,
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

impl TryFrom<String> for ChordSpec {
    type Error = ChordParseError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(Self::from_str(&s)?)
    }
}

impl fmt::Display for ChordSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut name = "".to_string();

        // Convert 1-indexed degree to 0-indexed
        let mut numeral = NUMERALS[(self.degree - 1) % 7].to_string();
        if self.mode == Mode::Minor || self.triad == Triad::Diminished {
            numeral = numeral.to_lowercase();
        }
        name.push_str(&numeral);

        let count = self.adj.abs() as usize;
        let octaves = count/12;
        let rem = count.rem_euclid(12);
        if self.adj < 0 {
            name.push_str(&std::iter::repeat("b").take(rem).collect::<String>());
        } else if self.adj > 0 {
            name.push_str(&std::iter::repeat("#").take(rem).collect::<String>());
        }

        match self.triad {
            Triad::Diminished => name.push('-'),
            Triad::Augmented => name.push('+'),
            Triad::Sus2 => name.push('_'),
            Triad::Sus4 => name.push('^'),
            Triad::Power => name.push('5'),
            Triad::Mode => {}
        }

        let exts = self.extensions.iter()
            .map(|ext| ext.to_string())
            .collect::<Vec<String>>();
        if exts.len() > 0 {
            name.push(':');
            name.push_str(&exts.join(","));
        }

        if let Some(bass_degree) = &self.bass_degree {
            name.push('/');
            name.push_str(&bass_degree.to_string());
        }

        if octaves != 0 {
            if self.adj < 0 {
                name.push_str(&format!("<{}", octaves));
            } else if self.adj > 0 {
                name.push_str(&format!(">{}", octaves));
            }
        }

        if let Some((degree, mode)) = self.rel_key {
            name.push('~');
            let numeral = NUMERALS[(degree - 1) % 7];
            if mode == Mode::Minor {
                name.push_str(&numeral.to_lowercase());
            } else {
                name.push_str(&numeral);
            }
        }
        write!(f, "{}", name)
    }
}


#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Chord {
    root: Note,
    intervals: Vec<Interval>
}

impl Chord {
    pub fn new(root: Note, intervals: Vec<isize>) -> Chord {
        Chord {
            root,
            intervals: intervals.into_iter()
                .map(Into::into).collect()
        }
    }

    /// Return the notes that make up this chord.
    pub fn notes(&self) -> Vec<Note> {
        let mut notes: Vec<Note> = self.intervals.iter().map(|intv| self.root + *intv).collect();
        notes.sort_by_key(|n| n.semitones);
        notes
    }

    pub fn describe_notes(&self) -> Vec<String> {
        self.notes().iter().map(|n| n.to_string()).collect()
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_chord_notes() {
        let chord = Chord::new(
            "C3".try_into().unwrap(),
            vec![0, 4, 7]);
        let notes = chord.notes();
        let expected = vec![Note {
            semitones: 27
        }, Note {
            semitones: 31
        }, Note {
            semitones: 34
        }];
        assert_eq!(notes.len(), expected.len());
        for (a, b) in notes.iter().zip(expected) {
            assert_eq!(*a, b);
        }
    }

    #[test]
    fn test_chord_spec_names() {
        let spec = ChordSpec::new(1, Mode::Major);
        assert_eq!(spec.to_string(), "I".to_string());

        let spec = ChordSpec::new(3, Mode::Minor);
        assert_eq!(spec.to_string(), "iii".to_string());

        let spec = ChordSpec::new(3, Mode::Minor).triad(Triad::Diminished);
        assert_eq!(spec.to_string(), "iii-".to_string());

        let spec = ChordSpec::new(3, Mode::Major).triad(Triad::Augmented);
        assert_eq!(spec.to_string(), "III+".to_string());

        let spec = ChordSpec::new(3, Mode::Minor).triad(Triad::Diminished)
            .add(7, 0);
        assert_eq!(spec.to_string(), "iii-:7".to_string());

        let spec = ChordSpec::new(3, Mode::Minor).triad(Triad::Diminished)
            .add(7, 0).bass(5, 0);
        assert_eq!(spec.to_string(), "iii-:7/5".to_string());

        let spec = ChordSpec::new(3, Mode::Minor).triad(Triad::Diminished)
            .add(7, 0).bass(5, 0).key_of(2, Mode::Minor);
        assert_eq!(spec.to_string(), "iii-:7/5~ii".to_string());

        let spec = ChordSpec::new(3, Mode::Minor).triad(Triad::Diminished)
            .add(7, 0).add(9, 0).bass(5, 0).key_of(2, Mode::Minor);
        assert_eq!(spec.to_string(), "iii-:7,9/5~ii".to_string());

        let spec = ChordSpec::new(1, Mode::Major).triad(Triad::Power);
        assert_eq!(spec.to_string(), "I5".to_string());

        let spec = ChordSpec::new(7, Mode::Major).adj(-1);
        assert_eq!(spec.to_string(), "VIIb".to_string());

        let spec = ChordSpec::new(1, Mode::Major).shift(1);
        assert_eq!(spec.to_string(), "I>1".to_string());

        let spec = ChordSpec::new(1, Mode::Major).shift(-1);
        assert_eq!(spec.to_string(), "I<1".to_string());
    }

    #[test]
    fn test_chord_intervals() {
        // Reference: <https://en.wikipedia.org/wiki/List_of_chords>
        let examples = vec![
            ("I", vec![0, 4, 7]),             // Major triad
            ("I:6", vec![0, 4, 7, 9]),         // Major 6th
            ("I:7", vec![0, 4, 7, 11]),        // Major 7th
            ("I:7,9", vec![0, 4, 7, 11, 14]),  // Major 9th

            ("i", vec![0, 3, 7]),              // Minor triad
            ("i:#6", vec![0, 3, 7, 9]),        // Minor 6th
            ("i:7", vec![0, 3, 7, 10]),        // Minor 7th
            ("i:7,#9", vec![0, 3, 7, 10, 14]), // Minor 9th

            ("I:b7", vec![0, 4, 7, 10]),       // Dominant 7th
            ("I:b7,9", vec![0, 4, 7, 10, 14]), // Dominant 9th

            ("I+", vec![0, 4, 8]),            // Augmented triad
            ("I+:b7", vec![0, 4, 8, 10]),      // Augmented 7th

            ("i-", vec![0, 3, 6]),            // Diminished triad
            ("i-:b7", vec![0, 3, 6, 9]),       // Diminished 7th
            ("i-:7", vec![0, 3, 6, 10]),       // Half-Diminished 7th

            ("I_", vec![0, 2, 7]),            // Sus 2
            ("I^", vec![0, 5, 7]),            // Sus 4

            ("I5", vec![0, 7]),               // Power
        ];
        for (name, expected) in examples {
            println!("Name: {:?}", name);
            let chord: ChordSpec = name.try_into().unwrap();
            let intervals = chord.intervals();
            println!(" Intervals: {:?}", intervals);
            println!(" Expected: {:?}", expected);
            assert_eq!(intervals.len(), expected.len());
            for (a, b) in intervals.iter().zip(expected) {
                assert_eq!(*a, b);
            }
        }
    }

    #[test]
    fn test_chord_for_keys() {
        let key = Key {
            root: "C3".try_into().unwrap(),
            mode: Mode::Major,
        };

        let spec = ChordSpec::new(1, Mode::Major).add(7, 0);
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

        let spec = ChordSpec::new(1, Mode::Major).bass(3, 0);
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
    fn test_chord_for_keys_rel_key() {
        let key = Key {
            root: "C3".try_into().unwrap(),
            mode: Mode::Major,
        };

        // Secondary dominant, this is V7/V in conventional notation
        let spec: ChordSpec = "V:b7~V".try_into().unwrap();
        let chord = spec.chord_for_key(&key);
        let notes = chord.notes();
        let expected = vec![Note {
            semitones: 41
        }, Note {
            semitones: 45
        }, Note {
            semitones: 48
        }, Note {
            semitones: 51
        }];
        assert_eq!(notes.len(), expected.len());
        for (a, b) in notes.iter().zip(expected) {
            assert_eq!(*a, b);
        }
    }

    #[test]
    fn test_chord_for_keys_adj() {
        let key = Key {
            root: "C3".try_into().unwrap(),
            mode: Mode::Major,
        };

        let spec: ChordSpec = "VIIb".try_into().unwrap();
        let chord = spec.chord_for_key(&key);
        let notes = chord.notes();
        let expected = vec![Note {
            semitones: 37
        }, Note {
            semitones: 41
        }, Note {
            semitones: 44
        }];
        assert_eq!(notes.len(), expected.len());
        for (a, b) in notes.iter().zip(expected) {
            assert_eq!(*a, b);
        }
    }

    #[test]
    fn test_parse_chord_spec() {
        let name = "III";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(3, Mode::Major);
        assert_eq!(spec, expected);

        let name = "III:7,9";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(3, Mode::Major).add(7, 0).add(9, 0);
        assert_eq!(spec, expected);

        let name = "iii:7,9";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(3, Mode::Minor).add(7, 0).add(9, 0);
        assert_eq!(spec, expected);

        let name = "iii-:7,9";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(3, Mode::Minor).triad(Triad::Diminished).add(7, 0).add(9, 0);
        assert_eq!(spec, expected);

        let name = "III+:7,9";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(3, Mode::Major).triad(Triad::Augmented).add(7, 0).add(9, 0);
        assert_eq!(spec, expected);

        let name = "III+:7,9/3";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(3, Mode::Major).triad(Triad::Augmented).add(7, 0).add(9, 0).bass(3, 0);
        assert_eq!(spec, expected);

        let name = "V_:7,9~ii";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(5, Mode::Major).triad(Triad::Sus2).add(7, 0).add(9, 0).key_of(2, Mode::Minor);
        assert_eq!(spec, expected);

        let name = "V^:7,9/5~ii";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(5, Mode::Major).triad(Triad::Sus4).add(7, 0).add(9, 0).key_of(2, Mode::Minor).bass(5, 0);
        assert_eq!(spec, expected);

        let name = "VIIb";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(7, Mode::Major).adj(-1);
        assert_eq!(spec, expected);

        let name = "I5";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(1, Mode::Major).triad(Triad::Power);
        assert_eq!(spec, expected);

        let name = "I>1";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(1, Mode::Major).shift(1);
        assert_eq!(spec, expected);

        let name = "I<1";
        let spec: ChordSpec = name.try_into().unwrap();
        let expected = ChordSpec::new(1, Mode::Major).shift(-1);
        assert_eq!(spec, expected);
    }

    #[test]
    fn test_parse_extension() {
        let ext: Extension = "7".try_into().unwrap();
        assert_eq!(ext, Extension {degree: 7, adj: 0});

        let ext: Extension = "b7".try_into().unwrap();
        assert_eq!(ext, Extension {degree: 7, adj: -1});

        let ext: Extension = "bb7".try_into().unwrap();
        assert_eq!(ext, Extension {degree: 7, adj: -2});

        let ext: Extension = "#7".try_into().unwrap();
        assert_eq!(ext, Extension {degree: 7, adj: 1});

        let ext: Extension = "b#7".try_into().unwrap();
        assert_eq!(ext, Extension {degree: 7, adj: 0});
    }

    #[test]
    fn test_chord_inversions() {
        let key = Key {
            root: "C3".try_into().unwrap(),
            mode: Mode::Major,
        };

        let cs: ChordSpec = "I".try_into().unwrap();
        let chord = cs.chord_for_key(&key);
        let notes: Vec<String> = chord.notes()
            .iter().map(|n| n.to_string()).collect();
        assert_eq!(notes, vec!["C3", "E3", "G3"]);

        // First inversion
        let cs: ChordSpec = "I/3".try_into().unwrap();
        let chord = cs.chord_for_key(&key);
        let notes: Vec<String> = chord.notes()
            .iter().map(|n| n.to_string()).collect();
        assert_eq!(notes, vec!["E3", "G3", "C4"]);

        // Second inversion
        let cs: ChordSpec = "I/5".try_into().unwrap();
        let chord = cs.chord_for_key(&key);
        let notes: Vec<String> = chord.notes()
            .iter().map(|n| n.to_string()).collect();
        assert_eq!(notes, vec!["G3", "C4", "E4"]);
    }

    #[test]
    fn test_cluster_chords() {
        let key = Key {
            root: "C3".try_into().unwrap(),
            mode: Mode::Major,
        };

        let cs: ChordSpec = "I:2".try_into().unwrap();
        let chord = cs.chord_for_key(&key);
        let notes: Vec<String> = chord.notes()
            .iter().map(|n| n.to_string()).collect();
        assert_eq!(notes, vec!["C3", "D3", "E3", "G3"]);
    }

    #[test]
    fn test_chord_octaves() {
        let key = Key {
            root: "C3".try_into().unwrap(),
            mode: Mode::Major,
        };

        let cs: ChordSpec = "I>1".try_into().unwrap();
        let chord = cs.chord_for_key(&key);
        let notes: Vec<String> = chord.notes()
            .iter().map(|n| n.to_string()).collect();
        assert_eq!(notes, vec!["C4", "E4", "G4"]);

        let cs: ChordSpec = "I:2>1".try_into().unwrap();
        let chord = cs.chord_for_key(&key);
        let notes: Vec<String> = chord.notes()
            .iter().map(|n| n.to_string()).collect();
        assert_eq!(notes, vec!["C4", "D4", "E4", "G4"]);

        let cs: ChordSpec = "I<1".try_into().unwrap();
        let chord = cs.chord_for_key(&key);
        let notes: Vec<String> = chord.notes()
            .iter().map(|n| n.to_string()).collect();
        assert_eq!(notes, vec!["C2", "E2", "G2"]);

        let cs: ChordSpec = "I:2<1".try_into().unwrap();
        let chord = cs.chord_for_key(&key);
        let notes: Vec<String> = chord.notes()
            .iter().map(|n| n.to_string()).collect();
        assert_eq!(notes, vec!["C2", "D2", "E2", "G2"]);
    }
}
