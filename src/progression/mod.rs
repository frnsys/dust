mod template;

pub use template::{ProgressionTemplate, ModeTemplate};
use crate::core::{Key, ChordSpec, Chord, Duration, voice_lead};

#[derive(Debug)]
pub struct Progression {
    pub resolution: Duration,

    // Progression sequence, one element
    // per tick. "None"s are rests.
    pub sequence: Vec<Option<ChordSpec>>,

    // Mapping of chords to their position
    // in the sequence,
    // e.g. chord_index[0] gives the position
    // of the first chord in the sequence,
    // so self.sequence[self.chord_index][0].unwrap()
    // will return the chord itself.
    pub chord_index: Vec<usize>,
}

impl Progression {
    pub fn new(sequence: Vec<Option<ChordSpec>>, resolution: Duration) -> Progression {
        Progression {
            resolution,
            chord_index: index_chords(&sequence),
            sequence,
        }
    }

    pub fn bars(&self) -> usize {
        self.sequence.len() / self.resolution.ticks_per_bar()
    }

    pub fn in_key(&self, key: &Key) -> Vec<Option<Chord>> {
        self.sequence.iter()
            .map(|cs| cs.as_ref().and_then(|c| Some(c.chord_for_key(key))))
            .collect()
    }

    pub fn chord(&self, chord_idx: usize) -> Option<&ChordSpec> {
        if chord_idx < self.chord_index.len() {
            let seq_idx = self.chord_index[chord_idx];
            self.sequence[seq_idx].as_ref()
        } else {
            None
        }
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
        self.update_chords();
    }

    pub fn insert_chord_at(&mut self, seq_idx: usize, chord: ChordSpec) {
        self.sequence[seq_idx] = Some(chord);
        self.update_chords();
    }

    pub fn update_chords(&mut self) {
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

    pub fn voice_lead(&self) -> Progression {
        let mut prog = Progression {
            resolution: self.resolution.clone(),
            chord_index: self.chord_index.clone(),
            sequence: self.sequence.clone()
        };
        if self.chord_index.is_empty() {
            prog
        } else {
            // Kind of messy, is there a cleaner way?
            let chords = self.chords().into_iter().cloned().collect();
            for (i, chord) in voice_lead(&chords).into_iter().enumerate() {
                prog.set_chord(i, chord);
            }
            prog
        }
    }
}

fn index_chords(seq: &Vec<Option<ChordSpec>>) -> Vec<usize> {
    seq.iter().enumerate()
        .filter_map(|(i, cs)| cs.as_ref().and(Some(i)))
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::core::Mode;

    #[test]
    fn test_voice_leading() {
        let prog = Progression::new(
            vec![
                None,
                Some("i".try_into().unwrap()),
                Some("VI".try_into().unwrap()),
                Some("III".try_into().unwrap()),
                Some("v".try_into().unwrap()),
            ],
            Duration::Eighth,
        );
        let vl_prog = prog.voice_lead();
        let key = Key {
            root: "A3".try_into().unwrap(),
            mode: Mode::Minor,
        };
        let expected = vec![
            "A3-C4-E4",
            "A3-C4-F4",
            "G3-C4-E4",
            "G3-B3-E4"
        ];
        for (cs, ex) in vl_prog.chords().iter().zip(expected) {
            let chord = cs.chord_for_key(&key).to_string();
            assert_eq!(ex, chord);
        }
    }
}
