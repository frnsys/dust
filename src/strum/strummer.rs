//! Turns strokes into timed, velocity-varied MIDI note events.
use super::{StrumParams, StrokeEvent, Stroke};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventKind {
    On(u8),
    Off,
}

/// A MIDI note on/off at a point in time. Times are in milliseconds
/// relative to whatever reference the caller used when rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteEvent {
    pub at_ms: f64,
    pub note: u8,
    pub kind: EventKind,
}

impl NoteEvent {
    pub fn on(at_ms: f64, note: u8, velocity: u8) -> NoteEvent {
        NoteEvent { at_ms, note, kind: EventKind::On(velocity) }
    }

    pub fn off(at_ms: f64, note: u8) -> NoteEvent {
        NoteEvent { at_ms, note, kind: EventKind::Off }
    }

    pub fn is_on(&self) -> bool {
        matches!(self.kind, EventKind::On(_))
    }
}

/// Sort events by time; note offs come before note ons at the same time
/// so that a re-struck string is always damped before it sounds again.
pub fn sort_events(events: &mut [NoteEvent]) {
    events.sort_by(|a, b| {
        a.at_ms.partial_cmp(&b.at_ms).unwrap()
            .then_with(|| a.is_on().cmp(&b.is_on()))
    });
}

/// Voice a chord across `strings` strings: starting from the lowest note,
/// stack chord tones upwards (repeating in higher octaves) until there are
/// enough notes. With `strings == 0` the chord's notes are used as-is.
pub fn voice(chord: &[u8], strings: usize) -> Vec<u8> {
    let mut notes: Vec<u8> = chord.to_vec();
    notes.sort_unstable();
    notes.dedup();
    if strings == 0 || notes.is_empty() {
        return notes;
    }
    let classes: BTreeSet<u8> = notes.iter().map(|n| n % 12).collect();
    let mut voicing = vec![];
    let mut n = notes[0];
    while voicing.len() < strings && n <= 127 {
        if classes.contains(&(n % 12)) {
            voicing.push(n);
        }
        n += 1;
    }
    voicing
}

pub struct Strummer {
    pub params: StrumParams,
    rng: StdRng,
    /// The current chord voiced across strings, lowest first.
    voicing: Vec<u8>,
    /// Notes that are currently ringing.
    sounding: BTreeSet<u8>,
    /// Whether the next bass note should be the alternate (second-lowest) string.
    bass_alt: bool,
}

impl Strummer {
    pub fn new(params: StrumParams, seed: u64) -> Strummer {
        Strummer {
            params,
            rng: StdRng::seed_from_u64(seed),
            voicing: vec![],
            sounding: BTreeSet::new(),
            bass_alt: false,
        }
    }

    pub fn from_entropy(params: StrumParams) -> Strummer {
        Strummer::new(params, rand::rng().random())
    }

    /// Forget any sounding notes without emitting note offs
    /// (e.g. after an all-notes-off was sent some other way).
    pub fn reset(&mut self) {
        self.voicing.clear();
        self.sounding.clear();
        self.bass_alt = false;
    }

    #[cfg(test)]
    pub fn sounding(&self) -> Vec<u8> {
        self.sounding.iter().copied().collect()
    }

    /// Standard normal sample (Box-Muller).
    fn gauss(&mut self) -> f64 {
        let u1: f64 = self.rng.random::<f64>().max(f64::MIN_POSITIVE);
        let u2: f64 = self.rng.random();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    /// Delay applied to off-beat subdivisions by the swing setting.
    fn swing_offset(&self, beat: f64, beat_ms: f64) -> f64 {
        let frac = beat.fract();
        let near = |x: f64| (frac - x).abs() < 1e-6;
        if near(0.5) {
            self.params.swing * beat_ms / 6.0
        } else if near(0.25) || near(0.75) {
            self.params.swing * beat_ms / 12.0
        } else {
            0.0
        }
    }

    /// Note offs for every sounding note at `t_ms`.
    pub fn release_all(&mut self, t_ms: f64) -> Vec<NoteEvent> {
        let events = self.sounding.iter().map(|n| NoteEvent::off(t_ms, *n)).collect();
        self.sounding.clear();
        events
    }

    /// Render one stroke of `chord` (as MIDI note numbers) nominally at `t_ms`.
    ///
    /// `beat_ms` is the current tempo and `gap_beats` the time until the next
    /// stroke, used to keep the strum from running into it. Events may be
    /// slightly earlier than `t_ms` (see `StrumParams::anchor`), and may
    /// include note offs for previously sounding notes.
    pub fn stroke(&mut self, ev: &StrokeEvent, chord: Option<&[u8]>, t_ms: f64, beat_ms: f64, gap_beats: f64) -> Vec<NoteEvent> {
        let p = self.params.clone();
        let t0 = t_ms
            + self.gauss() * p.timing_jitter_ms
            + if ev.stroke == Stroke::Up { p.up_late_ms } else { 0.0 }
            + self.swing_offset(ev.beat, beat_ms);

        // Re-voice if the chord changed. A chord change damps everything
        // that was ringing, since the fretting hand moves.
        let mut chord_changed = false;
        if let Some(chord) = chord {
            let voicing = voice(chord, p.strings);
            if voicing != self.voicing {
                self.voicing = voicing;
                self.bass_alt = false;
                chord_changed = true;
            }
        }
        let n = self.voicing.len();

        if ev.stroke == Stroke::Mute {
            let mut events = self.release_all(t0);
            if p.mute_hit && n > 0 {
                let velocity = p.velocity as f64 * p.mute_velocity
                    * if ev.accent { p.accent_velocity } else { 1.0 };
                let spread = (p.spread_ms * 0.5).min(p.max_spread_fraction * gap_beats * beat_ms);
                let strings: Vec<usize> = (0..n).collect();
                let (ons, _) = self.strike(&strings, t0, spread, velocity, &p);
                for on in ons {
                    events.push(NoteEvent::off(on.at_ms + p.mute_ms, on.note));
                    events.push(on);
                }
            }
            sort_events(&mut events);
            return events;
        }

        if n == 0 {
            return vec![];
        }

        // Which strings get hit, in the order they're hit.
        let strings: Vec<usize> = match ev.stroke {
            Stroke::Down => (0..n).collect(),
            Stroke::Up => {
                let k = if p.up_strings == 0 { n } else { p.up_strings.min(n) };
                (n - k..n).rev().collect()
            }
            Stroke::Bass => {
                let idx = if p.alternate_bass && self.bass_alt { 1.min(n - 1) } else { 0 };
                self.bass_alt = !self.bass_alt;
                vec![idx]
            }
            Stroke::Mute => unreachable!(),
        };

        let mut spread = p.spread_ms
            * if ev.stroke == Stroke::Up { p.up_spread_scale } else { 1.0 }
            * if ev.accent { p.accent_spread_scale } else { 1.0 }
            * (1.0 + self.gauss() * p.spread_jitter);
        spread = spread.clamp(0.0, (p.max_spread_fraction * gap_beats * beat_ms).max(0.0));

        let velocity = p.velocity as f64
            * match ev.stroke {
                Stroke::Up => p.up_velocity,
                Stroke::Bass => p.bass_velocity,
                _ => 1.0,
            }
            * if ev.accent { p.accent_velocity } else { 1.0 };

        let (ons, first_on) = self.strike(&strings, t0, spread, velocity, &p);

        let mut events = vec![];
        if chord_changed {
            events.extend(self.release_all(first_on - p.release_ms));
        } else {
            for on in &ons {
                if self.sounding.remove(&on.note) {
                    events.push(NoteEvent::off(on.at_ms - p.release_ms, on.note));
                }
            }
        }
        for on in ons {
            self.sounding.insert(on.note);
            events.push(on);
        }
        sort_events(&mut events);
        events
    }

    /// Note ons for hitting the given strings (indices into the voicing,
    /// in strike order) spread over `spread` ms around `t0`.
    /// Returns the events and the time of the first one.
    fn strike(&mut self, strings: &[usize], t0: f64, spread: f64, velocity: f64, p: &StrumParams) -> (Vec<NoteEvent>, f64) {
        let m = strings.len();
        let gap = if m > 1 { spread / (m - 1) as f64 } else { 0.0 };
        let mut offsets: Vec<f64> = (0..m)
            .map(|k| k as f64 * gap + self.gauss() * p.string_jitter * gap)
            .collect();
        offsets.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let shift = p.anchor * spread;

        let mut ons = Vec::with_capacity(m);
        for (k, idx) in strings.iter().enumerate() {
            let frac = if m > 1 { k as f64 / (m - 1) as f64 } else { 0.5 };
            let vel = velocity * (1.0 + p.velocity_gradient * (frac - 0.5))
                + self.gauss() * p.velocity_jitter;
            let vel = vel.round().clamp(1.0, 127.0) as u8;
            ons.push(NoteEvent::on(t0 + offsets[k] - shift, self.voicing[*idx], vel));
        }
        let first_on = ons.first().map(|e| e.at_ms).unwrap_or(t0);
        (ons, first_on)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const C: [u8; 3] = [60, 64, 67];
    const G: [u8; 3] = [55, 59, 62];
    const BEAT: f64 = 500.0;

    fn down(beat: f64) -> StrokeEvent {
        StrokeEvent { beat, stroke: Stroke::Down, accent: false }
    }
    fn up(beat: f64) -> StrokeEvent {
        StrokeEvent { beat, stroke: Stroke::Up, accent: false }
    }
    fn ons(events: &[NoteEvent]) -> Vec<&NoteEvent> {
        events.iter().filter(|e| e.is_on()).collect()
    }
    fn offs(events: &[NoteEvent]) -> Vec<&NoteEvent> {
        events.iter().filter(|e| !e.is_on()).collect()
    }

    #[test]
    fn test_voicing() {
        assert_eq!(voice(&C, 6), vec![60, 64, 67, 72, 76, 79]);
        assert_eq!(voice(&[67, 60, 64], 4), vec![60, 64, 67, 72]);
        assert_eq!(voice(&C, 0), vec![60, 64, 67]);
        // Inversions keep their bass note
        assert_eq!(voice(&[64, 67, 72], 5), vec![64, 67, 72, 76, 79]);
    }

    #[test]
    fn test_down_stroke_ascends_and_fans() {
        let mut s = Strummer::new(StrumParams::quantized(), 1);
        let events = s.stroke(&down(0.0), Some(&C), 1000.0, BEAT, 1.0);
        let ons = ons(&events);
        assert_eq!(ons.len(), 6);
        let notes: Vec<u8> = ons.iter().map(|e| e.note).collect();
        assert_eq!(notes, vec![60, 64, 67, 72, 76, 79]);
        // Strictly increasing times, evenly spaced over the spread, centered on the beat
        let times: Vec<f64> = ons.iter().map(|e| e.at_ms).collect();
        assert!(times.windows(2).all(|w| w[1] > w[0]));
        assert!((times[5] - times[0] - 35.0).abs() < 1e-9);
        assert!((times[0] - (1000.0 - 17.5)).abs() < 1e-9);
        // First stroke: nothing to damp
        assert!(offs(&events).is_empty());
    }

    #[test]
    fn test_up_stroke_descends_and_is_partial() {
        let mut s = Strummer::new(StrumParams::quantized(), 1);
        s.stroke(&down(0.0), Some(&C), 0.0, BEAT, 1.0);
        let events = s.stroke(&up(1.0), Some(&C), BEAT, BEAT, 1.0);
        let notes: Vec<u8> = ons(&events).iter().map(|e| e.note).collect();
        assert_eq!(notes, vec![79, 76, 72, 67]);
        // Only the re-struck strings are damped; the low strings keep ringing
        let damped: Vec<u8> = offs(&events).iter().map(|e| e.note).collect();
        assert_eq!(damped, vec![79, 76, 72, 67]);
        assert_eq!(s.sounding(), vec![60, 64, 67, 72, 76, 79]);
    }

    #[test]
    fn test_offs_precede_ons_for_same_note() {
        let mut s = Strummer::new(StrumParams::default(), 7);
        s.stroke(&down(0.0), Some(&C), 0.0, BEAT, 1.0);
        for i in 1..20 {
            let ev = if i % 2 == 0 { down(i as f64) } else { up(i as f64) };
            let events = s.stroke(&ev, Some(&C), i as f64 * BEAT, BEAT, 1.0);
            for on in ons(&events) {
                let off = offs(&events).into_iter().find(|e| e.note == on.note).unwrap();
                assert!(off.at_ms < on.at_ms, "off at {} not before on at {}", off.at_ms, on.at_ms);
            }
        }
    }

    #[test]
    fn test_chord_change_damps_everything() {
        let mut s = Strummer::new(StrumParams::quantized(), 1);
        s.stroke(&down(0.0), Some(&C), 0.0, BEAT, 1.0);
        let events = s.stroke(&up(1.0), Some(&G), BEAT, BEAT, 1.0);
        let damped: Vec<u8> = offs(&events).iter().map(|e| e.note).collect();
        assert_eq!(damped, vec![60, 64, 67, 72, 76, 79]);
        let first_on = ons(&events)[0].at_ms;
        assert!(offs(&events).iter().all(|e| e.at_ms < first_on));
    }

    #[test]
    fn test_velocities() {
        let mut s = Strummer::new(StrumParams::quantized(), 1);
        let vel = |events: &[NoteEvent]| -> Vec<u8> {
            ons(events).iter().map(|e| match e.kind { EventKind::On(v) => v, _ => 0 }).collect()
        };
        let d = vel(&s.stroke(&down(0.0), Some(&C), 0.0, BEAT, 1.0));
        let u = vel(&s.stroke(&up(1.0), Some(&C), BEAT, BEAT, 1.0));
        let acc = vel(&s.stroke(&StrokeEvent { beat: 2.0, stroke: Stroke::Down, accent: true }, Some(&C), 2.0 * BEAT, BEAT, 1.0));
        // Gradient: later strings a little louder
        assert!(d.windows(2).all(|w| w[1] >= w[0]));
        assert_eq!(d[0], (92.0_f64 * (1.0 - 0.075)).round() as u8);
        // Up strokes softer, accents louder
        let mean = |v: &[u8]| v.iter().map(|x| *x as f64).sum::<f64>() / v.len() as f64;
        assert!(mean(&u) < mean(&d));
        assert!(mean(&acc) > mean(&d));
        assert!(acc.iter().all(|v| *v <= 127));
    }

    #[test]
    fn test_spread_clamped_to_gap() {
        let mut s = Strummer::new(StrumParams::quantized(), 1);
        // Sixteenths at a fast tempo: 100ms between strokes, so a 35ms
        // spread is fine, but the clamp kicks in if the gap is tiny.
        let events = s.stroke(&down(0.0), Some(&C), 0.0, 200.0, 0.1);
        let times: Vec<f64> = ons(&events).iter().map(|e| e.at_ms).collect();
        assert!(times[5] - times[0] <= 0.6 * 20.0 + 1e-9);
    }

    #[test]
    fn test_bass_alternates_and_resets_on_chord_change() {
        let mut s = Strummer::new(StrumParams::quantized(), 1);
        let bass = |beat: f64| StrokeEvent { beat, stroke: Stroke::Bass, accent: false };
        let n = |events: Vec<NoteEvent>| ons(&events)[0].note;
        assert_eq!(n(s.stroke(&bass(0.0), Some(&C), 0.0, BEAT, 1.0)), 60);
        assert_eq!(n(s.stroke(&bass(1.0), Some(&C), BEAT, BEAT, 1.0)), 64);
        assert_eq!(n(s.stroke(&bass(2.0), Some(&C), 2.0 * BEAT, BEAT, 1.0)), 60);
        assert_eq!(n(s.stroke(&bass(3.0), Some(&G), 3.0 * BEAT, BEAT, 1.0)), 55);
    }

    #[test]
    fn test_mute_damps_and_hits() {
        let mut s = Strummer::new(StrumParams::quantized(), 1);
        s.stroke(&down(0.0), Some(&C), 0.0, BEAT, 1.0);
        let mute = StrokeEvent { beat: 1.0, stroke: Stroke::Mute, accent: false };
        let events = s.stroke(&mute, Some(&C), BEAT, BEAT, 1.0);
        // Everything ringing is damped, then a short hit on all strings which is itself damped
        assert_eq!(ons(&events).len(), 6);
        assert_eq!(offs(&events).len(), 12);
        assert!(s.sounding().is_empty());
        let hit = ons(&events)[0];
        let end = offs(&events).into_iter().filter(|e| e.note == hit.note).last().unwrap();
        assert!((end.at_ms - hit.at_ms - 35.0).abs() < 1e-9);

        let mut s = Strummer::new(StrumParams { mute_hit: false, ..StrumParams::quantized() }, 1);
        s.stroke(&down(0.0), Some(&C), 0.0, BEAT, 1.0);
        let events = s.stroke(&mute, Some(&C), BEAT, BEAT, 1.0);
        assert!(ons(&events).is_empty());
        assert_eq!(offs(&events).len(), 6);
    }

    #[test]
    fn test_rest_without_chord_is_silent() {
        let mut s = Strummer::new(StrumParams::default(), 1);
        assert!(s.stroke(&down(0.0), None, 0.0, BEAT, 1.0).is_empty());
    }

    #[test]
    fn test_deterministic_with_seed() {
        let run = || {
            let mut s = Strummer::new(StrumParams::default(), 42);
            let mut all = vec![];
            for i in 0..8 {
                let ev = if i % 2 == 0 { down(i as f64) } else { up(i as f64) };
                all.extend(s.stroke(&ev, Some(&C), i as f64 * BEAT, BEAT, 1.0));
            }
            all
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn test_humanization_statistics() {
        // Over many strokes, timing offsets should be centered on the grid
        // with roughly the configured spread, and never wildly off.
        let params = StrumParams { anchor: 0.0, ..StrumParams::default() };
        let mut s = Strummer::new(params, 3);
        let mut offsets = vec![];
        for i in 0..400 {
            let t = i as f64 * BEAT;
            let events = s.stroke(&down(i as f64), Some(&C), t, BEAT, 1.0);
            offsets.push(ons(&events)[0].at_ms - t);
        }
        let mean = offsets.iter().sum::<f64>() / offsets.len() as f64;
        let var = offsets.iter().map(|o| (o - mean).powi(2)).sum::<f64>() / offsets.len() as f64;
        assert!(mean.abs() < 1.5, "mean offset {}", mean);
        assert!((var.sqrt() - 6.0).abs() < 1.5, "std dev {}", var.sqrt());
        assert!(offsets.iter().all(|o| o.abs() < 30.0));
    }
}
