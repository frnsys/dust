use anyhow::Result;
use crate::core::{Key, Chord, ChordSpec, Duration};
use crate::progression::{Progression, ProgressionTemplate};

pub struct PlaybackState {
    pub tick: usize,
    pub clip: (usize, usize),

    // Params
    pub bars: usize,
    pub key: Key,
    pub note_duration: u64,
    pub resolution: Duration,

    pub progression: Progression,
}

impl PlaybackState {
    pub fn new(template: &ProgressionTemplate) -> PlaybackState {
        let bars = 2;
        let key = Key::default();
        let resolution = Duration::Eighth;
        let progression = template.gen_progression(&key.mode, bars, &resolution);

        PlaybackState {
            tick: 0,
            clip: (0, progression.sequence.len()),
            bars,
            key,
            resolution,
            note_duration: 5,
            progression,
        }
    }

    pub fn tick(&mut self) {
        self.tick += 1;
        if self.tick >= self.clip_len() {
            self.tick = 0;
        }
    }

    pub fn reset_tick(&mut self) {
        self.tick = 0;
    }

    pub fn clip_len(&self) -> usize {
        self.clip.1 - self.clip.0
    }

    pub fn clip_start(&self) -> usize {
        self.clip.0
    }

    pub fn reset_clip(&mut self) {
        self.clip = (0, self.progression.sequence.len());
    }

    pub fn has_loop(&self) -> bool {
        let (a, b) = self.clip;
        let a_clip = a > 0;
        let b_clip = b < self.progression.sequence.len();
        a_clip || b_clip
    }

    /// Generates and plays a new random progression.
    pub fn gen_progression(&mut self, template: &ProgressionTemplate) -> Result<()> {
        self.progression = template.gen_progression(&self.key.mode, self.bars, &self.resolution);
        self.reset_clip();
        Ok(())
    }

    /// Generates and plays a new random progression,
    /// starting with a specific chord.
    pub fn gen_progression_from_seed(&mut self, chord: &ChordSpec, template: &ProgressionTemplate) -> Result<()> {
        self.progression = template.gen_progression_from_seed(chord, &self.key.mode, self.bars, &self.resolution);
        self.reset_clip();
        Ok(())
    }

    /// The current chord (if any) for the current tick
    pub fn current_chord(&self) -> Option<(Chord, u64)> {
        let i = self.tick + self.clip_start();
        if let Some(chord_spec) = &self.progression.sequence[i] {
            Some((chord_spec.chord_for_key(&self.key), self.note_duration))
        } else {
            None
        }
    }
}
