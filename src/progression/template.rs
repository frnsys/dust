use rand::{Rng, seq::SliceRandom};
use std::collections::HashMap;
use serde::{Deserialize, Deserializer};
use crate::core::{Mode, ChordSpec, Duration};
use super::Progression;

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
            Mode::Major => self.major.next(chord),
            Mode::Minor => self.minor.next(chord)
        }
    }

    /// Generate a progression of chord specs starting with this chord spec.
    pub fn gen_progression_from_seed(&self, seed: &ChordSpec, mode: &Mode, bars: usize, resolution: &Duration) -> Progression  {
        let mut rng = rand::thread_rng();
        let timings = self.gen_timing(bars, resolution);
        let mut last = seed.clone();
        let template = match mode {
            Mode::Major => &self.major,
            Mode::Minor => &self.minor,
        };
        let mut prog: Vec<Option<ChordSpec>> = vec![];
        for has_chord in timings {
            if has_chord {
                let next = if prog.len() == 0 {
                    seed.clone()
                } else {
                    let cands = template.next(&last);
                    let next = cands.choose(&mut rng);
                    if next.is_none() {
                        self.rand_chord_for_mode(mode)
                    } else {
                        next.unwrap().clone()
                    }
                };

                last = next.clone();
                prog.push(Some(next));
            } else {
                prog.push(None);
            }
        }
        Progression::new(prog, *resolution)
    }

    /// Generate a progression of chord specs for a given mode.
    pub fn gen_progression(&self, mode: &Mode, bars: usize, resolution: &Duration) -> Progression {
        let seed = self.rand_chord_for_mode(mode);
        self.gen_progression_from_seed(&seed, mode, bars, resolution)
    }

    /// Generates random timings for chords in the progression.
    fn gen_timing(&self, bars: usize, resolution: &Duration) -> Vec<bool> {
        // Always start with a chord on the first beat
        let mut seq = vec![true];
        let mut rng = rand::thread_rng();
        let total = bars * resolution.ticks_per_bar();
        loop {
            let pause = rng.gen_range(0..resolution.ticks_per_bar());
            for _ in 0..pause {
                seq.push(false);
            }
            seq.push(true);
            if seq.len() > total {
                break;
            }
        }
        seq.drain(0..total).collect()
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
        let progression = template.gen_progression(&mode, bars, &Duration::Eighth);
        assert_eq!(progression.sequence.len(), bars * Duration::Eighth.ticks_per_bar());
    }
}