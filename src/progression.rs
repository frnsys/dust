use crate::key::Mode;
use crate::chord::ChordSpec;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use serde::Deserialize;

const TIMING_FACTORS: [f64; 8] = [1., 2., 3., 4., 5., 6., 7., 8.];

#[derive(Deserialize, PartialEq, Clone, Debug)]
pub struct ModeTemplate {
    pub starts: Vec<String>,
    pub pattern: HashMap<String, Vec<String>>,
}

#[derive(Deserialize, PartialEq, Clone, Debug)]
pub struct ProgressionTemplate {
    pub major: ModeTemplate,
    pub minor: ModeTemplate,
}

impl ProgressionTemplate {
    /// Chord progressions
    /// Return a list of candidate chord specs
    /// to follow this one.
    fn next(&self, chord: &ChordSpec, mode: &Mode) -> Vec<ChordSpec> {
        let default = vec![];
        let chord_name = chord.to_string();
        let cands = match mode {
            Mode::Major => {
                self.major.pattern.get(&chord_name).unwrap_or(&default)
            }
            Mode::Minor => {
                self.minor.pattern.get(&chord_name).unwrap_or(&default)
            }
        };
        cands.into_iter().map(|c| c.to_owned().try_into().unwrap()).collect()
    }

    /// Generate a progression of chord specs from this chord spec.
    pub fn gen_progression(&self, start: &ChordSpec, bars: usize, mode: &Mode) -> Vec<(ChordSpec, f64)> {
        let mut rng = rand::thread_rng();
        let mut progression = vec![];
        let mut last = start.clone();
        let timings = self.gen_timing(bars);
        for beat in &timings {
            let next = if progression.len() == 0 {
                start.clone()
            } else {
                let cands = self.next(&last, mode);
                let next = cands.choose(&mut rng);
                if next.is_none() {
                    break;
                } else {
                    next.unwrap().clone()
                }
            };

            last = next.clone();
            progression.push((next, *beat));
        }
        progression
    }

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

    pub fn rand_chord_for_mode(&self, mode: &Mode) -> ChordSpec {
        let mut rng = rand::thread_rng();
        let cands = match mode {
            Mode::Major => &self.major.starts,
            Mode::Minor => &self.minor.starts
        };
        // TODO clean this up
        (*cands.choose(&mut rng).unwrap()).to_owned().try_into().unwrap()
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use crate::key::Mode;
    use crate::chord::{ChordSpec, Quality};

    #[test]
    fn test_chord_progression() {
        let bars = 4;
        let mode = Mode::Major;
        let start = ChordSpec::new(1, Quality::Major);
        let template = ProgressionTemplate {
            major: ModeTemplate {
                starts: vec![],
                pattern: HashMap::default()
            },
            minor: ModeTemplate {
                starts: vec![],
                pattern: HashMap::default()
            }
        };
        let progression = template.gen_progression(&start, bars, &mode);
        assert_eq!(progression.len(), bars);
    }
}
