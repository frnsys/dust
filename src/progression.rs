use rand::seq::SliceRandom;
use serde::{Deserialize, Deserializer};
use crate::core::{Key, Mode, ChordSpec, Chord};
use std::collections::HashMap;

const BEATS_PER_BAR: usize = 4; // Assume 4/4 time
const TIMING_FACTORS: [f64; 8] = [1., 2., 3., 4., 5., 6., 7., 8.];

pub struct Progression {
    pub bars: usize,
    pub time_unit: f64,

    // Progression sequence, one element
    // per tick. "None"s are rests.
    pub sequence: Vec<Option<ChordSpec>>,

    // Mapping of chords to their position
    // in the sequence,
    // e.g. chord_index[0] gives the position
    // of the first chord in the sequence,
    // so self.sequence[self.chord_index][0].unwrap()
    // will return the chord itself.
    chord_index: Vec<usize>,
}

/// Convert a timed progression to a sequence
fn timed_to_sequence(bars: usize, time_unit: f64, sequence: &Vec<(ChordSpec, f64)>) -> Vec<Option<ChordSpec>> {
    let mut seq = vec![];
    let mut last_beat = 0.;
    for (i, (cs, elapsed)) in sequence.iter().enumerate() {
        let beat = last_beat + elapsed;
        let n_rests = if i == 0 {
            (beat/time_unit) as usize
        } else {
            ((beat - last_beat)/time_unit) as usize - 1
        };
        for _ in 0..n_rests {
            seq.push(None);
        }
        last_beat = beat;
        seq.push(Some(cs.clone()));

        // Fill out to end if necessary
        if i == sequence.len() - 1 {
            let total_beats = (bars * BEATS_PER_BAR) as f64;
            let n_rests = ((total_beats - beat)/time_unit) as usize - 1;
            for _ in 0..n_rests {
                seq.push(None);
            }
        }
    }
    seq
}

fn index_chords(seq: &Vec<Option<ChordSpec>>) -> Vec<usize> {
    seq.iter().enumerate()
        .filter_map(|(i, cs)| cs.as_ref().and(Some(i)))
        .collect()
}

impl Progression {
    pub fn new(sequence: Vec<Option<ChordSpec>>, bars: usize, time_unit: f64) -> Progression {
        Progression {
            bars,
            time_unit,
            chord_index: index_chords(&sequence),
            sequence,
        }
    }

    pub fn in_key(&self, key: &Key) -> Vec<Option<Chord>> {
        self.sequence.iter()
            .map(|cs| cs.as_ref().and_then(|c| Some(c.chord_for_key(key))))
            .collect()
    }

    pub fn chord(&self, chord_idx: usize) -> Option<&ChordSpec> {
        let seq_idx = self.chord_index[chord_idx];
        self.sequence[seq_idx].as_ref()
    }

    pub fn chords(&self) -> Vec<&ChordSpec> {
        self.chord_index.iter()
            .filter_map(|c_idx| self.sequence[*c_idx].as_ref())
            .collect()
    }

    pub fn set_chord(&mut self, chord_idx: usize, chord: ChordSpec) {
        let seq_idx = self.chord_index[chord_idx];
        self.sequence[seq_idx] = Some(chord);
    }

    pub fn prev_chord(&self, chord_idx: usize) -> &ChordSpec {
        let idx = chord_idx as isize - 1;
        let idx = idx.rem_euclid(self.chord_index.len() as isize) as usize;
        let seq_idx = self.chord_index[idx];
        &self.sequence[seq_idx].as_ref().unwrap()
    }

    pub fn delete_chord_at(&mut self, seq_idx: usize) {
        self.sequence[seq_idx] = None;
        self.chord_index = index_chords(&self.sequence);
    }

    pub fn insert_chord_at(&mut self, seq_idx: usize, chord: ChordSpec) {
        self.sequence[seq_idx] = Some(chord);
        self.chord_index = index_chords(&self.sequence);
    }

    pub fn seq_idx_to_chord_idx(&self, seq_idx: usize) -> usize {
        // Bleh kind of hacky
        let mut chord_idx = 0;
        for (i, tick) in self.sequence.iter().enumerate() {
            if i == seq_idx {
                break;
            }
            if tick.is_some() {
                chord_idx += 1;
            }
        }
        chord_idx
    }
}

#[derive(Deserialize, PartialEq, Clone, Debug)]
pub struct ModeTemplate {
    #[serde(deserialize_with = "from_progression")]
    patterns: Vec<Vec<ChordSpec>>,

    #[serde(skip_deserializing)]
    transitions: HashMap<String, Vec<ChordSpec>>
}

impl ModeTemplate {
    /// Update the chord transition matrix
    pub fn update_transitions(&mut self) {
        self.transitions.clear();
        for pattern in &self.patterns {
            for (i, chord) in pattern.iter().enumerate() {
                let next = self.transitions.entry(chord.to_string()).or_insert(vec![]);

                // Candidate following chords can be:
                // - the chord itself (repeat)
                // - the chord before
                // - the chord after
                next.push(chord.clone());
                if i == 0 {
                    next.push(pattern[pattern.len() - 1].clone());
                    next.push(pattern[i+1].clone());
                } else if i == pattern.len() - 1 {
                    next.push(pattern[0].clone());
                    next.push(pattern[i-1].clone());
                } else {
                    next.push(pattern[i-1].clone());
                    next.push(pattern[i+1].clone());
                }
            }
        }
    }

    /// Get possible chords to follow the given chord
    pub fn next(&self, chord: &ChordSpec) -> Vec<ChordSpec> {
        let default = vec![];
        let chord_name = chord.to_string();
        self.transitions.get(&chord_name).unwrap_or(&default).clone()
    }
}

/// Lets us write progressions as space-separated strings in yaml,
/// e.g. "I ii VI" instead of "[I, ii, VI]"
fn from_progression<'de, D>(deserializer: D) -> Result<Vec<Vec<ChordSpec>>, D::Error>
where
    D: Deserializer<'de>,
{
    let patterns: Vec<String> = Deserialize::deserialize(deserializer)?;
    Ok(patterns.iter().map(|s| {
        s.split(" ").map(|cs| cs.try_into().unwrap()).collect()
    }).collect())
}

#[derive(Deserialize, PartialEq, Clone, Debug)]
pub struct ProgressionTemplate {
    major: ModeTemplate,
    minor: ModeTemplate,

    // How many divisions per bar,
    // e.g. 8 is eighth notes
    pub resolution: usize,
}

impl ProgressionTemplate {
    /// Update the mode chord transition matrix for each mode
    pub fn update_transitions(&mut self) {
        self.major.update_transitions();
        self.minor.update_transitions();
    }

    /// Chord progressions
    /// Return a list of candidate chord specs
    /// to follow this one.
    pub fn next(&self, chord: &ChordSpec, mode: &Mode) -> Vec<ChordSpec> {
        match mode {
            Mode::Major => self.major.next(chord),
            Mode::Minor => self.minor.next(chord)
        }
    }

    /// The base timing unit, in beats.
    fn time_unit(&self) -> f64 {
        // This is expressed in terms of beats, rather than bars,
        // so an eighth note converts to a half of a beat
        BEATS_PER_BAR as f64/self.resolution as f64
    }

    /// Generate a progression of chord specs from this chord spec.
    pub fn gen_progression(&self, bars: usize, mode: &Mode) -> Progression  {
        let mut rng = rand::thread_rng();
        let pattern = self.rand_pattern(mode);
        let n_chords = pattern.len() as isize;
        let mut progression: Vec<(ChordSpec, f64)> = vec![];
        let timings = self.gen_timing(bars);
        let mut i: isize = 0;
        for beat in &timings {
            // Can go back one, repeat, or go forward one
            let cands = vec![
                (i-1).rem_euclid(n_chords) as usize,
                (i).rem_euclid(n_chords) as usize,
                (i+1).rem_euclid(n_chords) as usize,
            ];
            // Can unwrap because we know there will be 3 candidates
            let next_idx = cands.choose(&mut rng).unwrap();
            let next = pattern[*next_idx].clone();
            progression.push((next, *beat));
            i = *next_idx as isize;
        }
        Progression::new(timed_to_sequence(bars, self.time_unit(), &progression), bars, self.time_unit())
    }

    /// Generates random timings for chords in the progression.
    fn gen_timing(&self, bars: usize) -> Vec<f64> {
        let mut total = 0.;

        // Always start with a chord on the first beat
        let mut timings = vec![0.];
        let mut rng = rand::thread_rng();
        let time_unit = self.time_unit();
        let total_beats = (bars * BEATS_PER_BAR) as f64;
        loop {
            let factor = TIMING_FACTORS.choose(&mut rng).unwrap();
            let beats = factor * time_unit;
            total += beats;
            if total >= total_beats {
                break;
            }
            timings.push(beats);
        }
        timings
    }

    /// Randomly chooses a pattern given a mode.
    pub fn rand_pattern(&self, mode: &Mode) -> Vec<ChordSpec> {
        let mut rng = rand::thread_rng();
        let cands = match mode {
            Mode::Major => &self.major.patterns,
            Mode::Minor => &self.minor.patterns
        };
        cands.choose(&mut rng).unwrap().clone()
    }

    /// Randomly chooses a chord given a mode.
    pub fn rand_chord_for_mode(&self, mode: &Mode) -> ChordSpec {
        let mut rng = rand::thread_rng();
        let cands = self.rand_pattern(mode);
        cands.choose(&mut rng).unwrap().clone()
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_chord_progression() {
        let bars = 4;
        let mode = Mode::Major;
        let template = ProgressionTemplate {
            resolution: 8,
            major: ModeTemplate {
                patterns: vec![vec![
                    "I".try_into().unwrap(),
                    "V".try_into().unwrap(),
                    "vi".try_into().unwrap(),
                    "IV".try_into().unwrap(),
                ]],
                transitions: HashMap::default()
            },
            minor: ModeTemplate {
                patterns: vec![],
                transitions: HashMap::default()
            }
        };
        let progression = template.gen_progression(bars, &mode);
        assert_eq!(progression.sequence.len(), bars * template.resolution);
    }

    #[test]
    fn test_timed_to_sequence() {
        let bars = 1;
        let template = ProgressionTemplate {
            resolution: 8,
            major: ModeTemplate {
                patterns: vec![vec![
                    "I".try_into().unwrap(),
                    "V".try_into().unwrap(),
                    "vi".try_into().unwrap(),
                    "IV".try_into().unwrap(),
                ]],
                transitions: HashMap::default()
            },
            minor: ModeTemplate {
                patterns: vec![],
                transitions: HashMap::default()
            }
        };
        let chord: ChordSpec = "I".try_into().unwrap();
        // At resolution=8 the smallest unit is 0.5
        // aka half a beat aka half a quarter note
        let progression = vec![
            (chord.clone(), 0.5), // 0.5
            (chord.clone(), 1.),  // 1.5
            (chord.clone(), 2.0), // 3.5
        ];
        let sequence = timed_to_sequence(bars, template.time_unit(), &progression);
        let expected = vec![
            None,
            Some(chord.clone()),
            None,
            Some(chord.clone()),
            None,
            None,
            None,
            Some(chord.clone()),
        ];
        assert_eq!(sequence.len(), template.resolution * bars);
        assert_eq!(sequence, expected);
    }
}
