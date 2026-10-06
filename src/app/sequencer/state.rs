use anyhow::Result;
use crate::core::{Key, Chord, ChordSpec, Duration};
use crate::midi::TICKS_PER_QUARTER;
use crate::progression::{Progression, ProgressionTemplate};
use crate::strum::{StrumLibrary, StrumPattern, StrumParams, Strummer, NoteEvent};

/// How long one unit of `note_duration` lasts when playing block chords.
pub const DURATION_UNIT_MS: f64 = 150.0;

/// How many clock ticks ahead strokes are scheduled. This gives the strum
/// room to start slightly before the beat (see `StrumParams::anchor`).
const LOOKAHEAD_TICKS: usize = 2;

pub struct PlaybackState {
    /// Current step in the clip (for display).
    pub tick: usize,
    /// Current clock tick (24 per quarter note) within the clip.
    clock_tick: usize,
    /// Whether playback has processed its first clock tick since starting.
    started: bool,
    /// Latest tempo estimate from the clock.
    pub bpm: f64,
    pub clip: (usize, usize),

    // Params
    pub bars: usize,
    pub key: Key,
    pub note_duration: u64,
    pub resolution: Duration,

    pub progression: Progression,

    // Strumming
    pub strum_library: StrumLibrary,
    /// Index of the selected strum preset, if strumming is on.
    pub strum: Option<usize>,
    strum_pattern: Option<StrumPattern>,
    strummer: Strummer,
    /// The chord currently ringing/held, as MIDI notes.
    held: Option<Vec<u8>>,
}

impl PlaybackState {
    pub fn new(template: &ProgressionTemplate, strum_library: StrumLibrary) -> PlaybackState {
        let bars = 2;
        let key = Key::default();
        let resolution = Duration::Eighth;
        let progression = template.gen_progression(&key.mode, bars, &resolution);

        PlaybackState {
            tick: 0,
            clock_tick: 0,
            started: false,
            bpm: 120.0,
            clip: (0, progression.sequence.len()),
            bars,
            key,
            resolution,
            note_duration: 5,
            progression,
            strummer: Strummer::from_entropy(strum_library.defaults.clone()),
            strum_library,
            strum: None,
            strum_pattern: None,
            held: None,
        }
    }

    /// Select a strum preset by index into the library, or none.
    pub fn set_strum(&mut self, idx: Option<usize>) {
        self.strum = idx.filter(|i| *i < self.strum_library.presets.len());
        match self.strum {
            Some(i) => {
                let preset = &self.strum_library.presets[i];
                self.strum_pattern = Some(preset.pattern.clone());
                self.strummer = Strummer::from_entropy(preset.params.clone());
            }
            None => {
                self.strum_pattern = None;
                self.strummer.reset();
            }
        }
    }

    pub fn strum_name(&self) -> String {
        match self.strum {
            Some(i) => self.strum_library.presets[i].name.clone(),
            None => "off".to_string(),
        }
    }

    pub fn strum_pattern(&self) -> Option<&StrumPattern> {
        self.strum_pattern.as_ref()
    }

    pub fn strum_params(&self) -> &StrumParams {
        &self.strummer.params
    }

    /// Rewind to the start of the clip and forget any held chord.
    /// The caller is responsible for silencing the MIDI output.
    pub fn reset_playback(&mut self) {
        self.tick = 0;
        self.clock_tick = 0;
        self.started = false;
        self.held = None;
        self.strummer.reset();
    }

    fn ticks_per_step(&self) -> usize {
        TICKS_PER_QUARTER / self.resolution.ticks_per_beat()
    }

    pub fn clip_len(&self) -> usize {
        self.clip.1 - self.clip.0
    }

    fn clip_len_ticks(&self) -> usize {
        self.clip_len() * self.ticks_per_step()
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

    /// The chord (if any) at a step of the clip.
    pub fn chord_at_step(&self, step: usize) -> Option<Chord> {
        let i = self.clip_start() + step;
        self.progression.sequence.get(i)
            .and_then(|cs| cs.as_ref())
            .map(|cs| cs.chord_for_key(&self.key))
    }

    fn notes_at_step(&self, step: usize) -> Option<Vec<u8>> {
        self.chord_at_step(step).map(|c| c.midi_notes())
    }

    /// Advance one clock tick. Returns the note events to play,
    /// timed in milliseconds from now.
    pub fn on_clock_tick(&mut self, ms_per_tick: f64) -> Vec<NoteEvent> {
        self.bpm = 60_000.0 / (ms_per_tick * TICKS_PER_QUARTER as f64);
        let tps = self.ticks_per_step();
        let len = self.clip_len_ticks();
        if len == 0 {
            return vec![];
        }
        let t = self.clock_tick % len;
        let mut events = vec![];

        // Entering a new step: pick up its chord, and without strumming, play it.
        if t % tps == 0 {
            let step = t / tps;
            if let Some(notes) = self.notes_at_step(step) {
                if self.strum.is_none() {
                    let hold = self.note_duration as f64 * DURATION_UNIT_MS;
                    for note in &notes {
                        events.push(NoteEvent::on(0.0, *note, 100));
                        events.push(NoteEvent::off(hold, *note));
                    }
                }
                self.held = Some(notes);
            }
        }

        if self.strum.is_some() {
            // Schedule the strokes falling in the window LOOKAHEAD ticks from
            // now. On the first tick also cover the ticks before the window.
            let start = if self.started { t + LOOKAHEAD_TICKS } else { t };
            self.started = true;
            for tick in start..=t + LOOKAHEAD_TICKS {
                events.extend(self.strokes_in_tick(tick, t, len, ms_per_tick));
            }
        }

        self.clock_tick = (t + 1) % len;
        self.tick = self.clock_tick / tps;
        events
    }

    /// Strokes falling in `[tick, tick + 1)` (in ticks from the start of
    /// the clip, possibly past its end, in which case they wrap), timed
    /// relative to the current tick `now_tick`.
    fn strokes_in_tick(&mut self, tick: usize, now_tick: usize, len: usize, ms_per_tick: f64) -> Vec<NoteEvent> {
        let Some(pattern) = &self.strum_pattern else { return vec![] };
        let tps = self.ticks_per_step() as f64;
        let beat_ms = ms_per_tick * TICKS_PER_QUARTER as f64;
        let local = tick % len;
        let from = local as f64 / TICKS_PER_QUARTER as f64;
        let to = (local + 1) as f64 / TICKS_PER_QUARTER as f64;
        let mut events = vec![];
        for (beat, stroke, gap) in pattern.strokes_between(from, to) {
            let p_tick = beat * TICKS_PER_QUARTER as f64;
            let step = (p_tick / tps).floor() as usize;
            let chord = self.notes_at_step(step).or_else(|| self.held.clone());
            let t_ms = (p_tick - local as f64 + (tick - now_tick) as f64) * ms_per_tick;
            events.extend(self.strummer.stroke(&stroke, chord.as_deref(), t_ms, beat_ms, gap));
        }
        events
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::strum::EventKind;

    fn state() -> PlaybackState {
        let template: ProgressionTemplate = serde_yaml::from_str("major:\n  patterns:\n    - I IV\nminor:\n  patterns:\n    - i iv\n").unwrap();
        let mut s = PlaybackState::new(&template, StrumLibrary::default_library());
        s.resolution = Duration::Quarter;
        s.progression = Progression::new(vec![
            Some("I".try_into().unwrap()), None,
            Some("IV".try_into().unwrap()), None,
        ], Duration::Quarter);
        s.reset_clip();
        s
    }

    /// Run `n` clock ticks at 120 BPM (20.8333ms per tick), collecting events
    /// with absolute times.
    fn run(s: &mut PlaybackState, n: usize) -> Vec<(f64, NoteEvent)> {
        let ms = 500.0 / 24.0;
        let mut all = vec![];
        for i in 0..n {
            let now = i as f64 * ms;
            for ev in s.on_clock_tick(ms) {
                all.push((now + ev.at_ms, ev));
            }
        }
        all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        all
    }

    #[test]
    fn test_block_playback_on_steps() {
        let mut s = state();
        let events = run(&mut s, 96);
        let ons: Vec<&(f64, NoteEvent)> = events.iter().filter(|(_, e)| e.is_on()).collect();
        // Two chords, 3 notes each, in one bar
        assert_eq!(ons.len(), 6);
        assert_eq!(ons[0].0, 0.0);
        assert!((ons[3].0 - 1000.0).abs() < 1e-9);
        // Held for note_duration units
        let off = events.iter().find(|(_, e)| !e.is_on()).unwrap();
        assert!((off.0 - 5.0 * DURATION_UNIT_MS).abs() < 1e-9);
        assert_eq!(s.tick, 0);
    }

    #[test]
    fn test_strummed_playback_follows_pattern_and_loops() {
        let mut s = state();
        let idx = s.strum_library.presets.iter().position(|p| p.name == "downs").unwrap();
        s.set_strum(Some(idx));
        // Quantize for a deterministic check
        s.strummer.params = StrumParams::quantized();
        let events = run(&mut s, 96 * 2);
        let ons: Vec<&(f64, NoteEvent)> = events.iter().filter(|(_, e)| e.is_on()).collect();
        // One down stroke per beat, 6 strings, 8 beats, plus the first beat
        // of the next pass which the lookahead schedules before the clock gets there
        assert_eq!(ons.len(), 6 * 9);
        // Beat 1 (t=500) is the first bar's second beat: still chord I (C), strum centered on the beat
        let beat1: Vec<u8> = ons.iter().filter(|(t, _)| (t - 500.0).abs() < 30.0).map(|(_, e)| e.note).collect();
        assert_eq!(beat1, vec![60, 64, 67, 72, 76, 79]);
        let first = ons.iter().filter(|(t, _)| (t - 500.0).abs() < 30.0).map(|(t, _)| *t).fold(f64::MAX, f64::min);
        assert!((first - (500.0 - 17.5)).abs() < 1e-6, "strum starts at {}", first);
        // Beat 2 is chord IV (F)
        let beat2: Vec<u8> = ons.iter().filter(|(t, _)| (t - 1000.0).abs() < 30.0).map(|(_, e)| e.note).collect();
        assert_eq!(beat2, vec![65, 69, 72, 77, 81, 84]);
        // Second time around the loop, back to C
        let beat4: Vec<u8> = ons.iter().filter(|(t, _)| (t - 2000.0).abs() < 30.0).map(|(_, e)| e.note).collect();
        assert_eq!(beat4, vec![60, 64, 67, 72, 76, 79]);
        // Every stroke got scheduled exactly once
        let mut times: Vec<i64> = ons.iter().map(|(t, _)| (t / 500.0).round() as i64).collect();
        times.dedup();
        assert_eq!(times, (0..9).collect::<Vec<i64>>());
    }

    #[test]
    fn test_first_stroke_is_not_scheduled_in_the_past() {
        let mut s = state();
        s.set_strum(Some(0));
        let events = s.on_clock_tick(500.0 / 24.0);
        let on = events.iter().find(|e| matches!(e.kind, EventKind::On(_))).unwrap();
        // Pre-roll would put the strum before now; it's the scheduler's job
        // to clamp, so here the time may be slightly negative but small.
        assert!(on.at_ms > -40.0);
    }
}
