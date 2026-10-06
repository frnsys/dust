//! Offline rendering of chord sequences to note events,
//! used for MIDI export and for listening tests.
use super::{Strummer, StrumPattern, StrumParams, NoteEvent, sort_events};

/// Render `repeats` passes over `steps` with a strum pattern. Empty steps
/// hold the previous chord; a stroke before the first chord is silent.
pub fn render_strummed(steps: &[Option<Vec<u8>>], steps_per_beat: usize, pattern: &StrumPattern, params: &StrumParams, bpm: f64, repeats: usize, seed: u64) -> Vec<NoteEvent> {
    let beat_ms = 60_000.0 / bpm;
    let loop_beats = steps.len() as f64 / steps_per_beat as f64;
    let mut strummer = Strummer::new(params.clone(), seed);
    let mut events = vec![];
    let mut held: Option<Vec<u8>> = None;
    for rep in 0..repeats {
        let rep_offset = rep as f64 * loop_beats;
        for (beat, stroke, gap) in pattern.strokes_between(0.0, loop_beats) {
            let step = (beat * steps_per_beat as f64).floor() as usize;
            if let Some(Some(chord)) = steps.get(step) {
                held = Some(chord.clone());
            }
            let t = (rep_offset + beat) * beat_ms;
            events.extend(strummer.stroke(&stroke, held.as_deref(), t, beat_ms, gap));
        }
    }
    events.extend(strummer.release_all(repeats as f64 * loop_beats * beat_ms));
    sort_events(&mut events);
    events
}

/// Render `repeats` passes over `steps` as plain block chords, each held
/// for `hold_ms`, which is how the sequencer plays without strumming.
pub fn render_block(steps: &[Option<Vec<u8>>], steps_per_beat: usize, bpm: f64, hold_ms: f64, velocity: u8, repeats: usize) -> Vec<NoteEvent> {
    let step_ms = 60_000.0 / bpm / steps_per_beat as f64;
    let mut events = vec![];
    for rep in 0..repeats {
        for (i, step) in steps.iter().enumerate() {
            if let Some(chord) = step {
                let t = (rep * steps.len() + i) as f64 * step_ms;
                for note in chord {
                    events.push(NoteEvent::on(t, *note, velocity));
                    events.push(NoteEvent::off(t + hold_ms, *note));
                }
            }
        }
    }
    sort_events(&mut events);
    events
}

#[cfg(test)]
mod test {
    use super::*;

    fn c_f_g() -> Vec<Option<Vec<u8>>> {
        vec![
            Some(vec![60, 64, 67]), None, None, None,
            Some(vec![53, 57, 60]), None, None, None,
            Some(vec![55, 59, 62]), None, None, None,
            None, None, None, None,
        ]
    }

    #[test]
    fn test_strummed_render_follows_chords() {
        let pattern: StrumPattern = "d u d u".parse().unwrap();
        let params = StrumParams { strings: 3, up_strings: 0, ..StrumParams::quantized() };
        let events = render_strummed(&c_f_g(), 1, &pattern, &params, 120.0, 2, 1);
        let ons: Vec<&NoteEvent> = events.iter().filter(|e| e.is_on()).collect();
        // 16 strokes per pass x 3 notes x 2 passes
        assert_eq!(ons.len(), 16 * 3 * 2);
        // Beat 0: C, beat 4: F, beat 8: G, beats 12-15 hold G, next pass C again
        let notes_at = |beat: f64| -> Vec<u8> {
            let t = beat * 500.0;
            let mut v: Vec<u8> = ons.iter().filter(|e| (e.at_ms - t).abs() < 40.0).map(|e| e.note).collect();
            v.sort_unstable();
            v
        };
        assert_eq!(notes_at(0.0), vec![60, 64, 67]);
        assert_eq!(notes_at(4.0), vec![53, 57, 60]);
        assert_eq!(notes_at(13.0), vec![55, 59, 62]);
        assert_eq!(notes_at(16.0), vec![60, 64, 67]);
        // Everything is released by the end
        let mut sounding = std::collections::HashSet::new();
        for e in &events {
            if e.is_on() { sounding.insert(e.note); } else { sounding.remove(&e.note); }
        }
        assert!(sounding.is_empty());
    }

    #[test]
    fn test_block_render() {
        let events = render_block(&c_f_g(), 2, 120.0, 100.0, 100, 1);
        let ons: Vec<&NoteEvent> = events.iter().filter(|e| e.is_on()).collect();
        assert_eq!(ons.len(), 9);
        assert_eq!(ons[3].at_ms, 4.0 * 250.0);
        assert_eq!(events.iter().filter(|e| !e.is_on()).count(), 9);
    }
}
