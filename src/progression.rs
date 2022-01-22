use rand::seq::SliceRandom;
use serde::{Deserialize, Deserializer};
use crate::core::{Mode, ChordSpec};
use std::collections::HashMap;

const TIMING_FACTORS: [f64; 8] = [1., 2., 3., 4., 5., 6., 7., 8.];

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
            Mode::Major => {
                self.major.next(chord)
            }
            Mode::Minor => {
                self.minor.next(chord)
            }
        }
    }

    /// Generate a progression of chord specs from this chord spec.
    pub fn gen_progression(&self, bars: usize, mode: &Mode) -> Vec<(ChordSpec, f64)> {
        let mut rng = rand::thread_rng();
        let pattern = self.rand_pattern(mode);
        let n_chords = pattern.len() as isize;
        let mut progression = vec![];
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
        progression
    }

    /// Generates random timings for chords in the progression.
    fn gen_timing(&self, bars: usize) -> Vec<f64> {
        let mut total = 0.;
        let mut timings = vec![];
        let mut rng = rand::thread_rng();
        loop {
            let factor = TIMING_FACTORS.choose(&mut rng).unwrap();
            let beats = factor * 0.25;
            total += beats;
            if total >= bars as f64 {
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
        assert!(progression.len() > 1);
    }
}
