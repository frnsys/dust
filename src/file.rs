use midly::{
    Smf, Header, Format, Timing,
    TrackEvent, TrackEventKind,
    MidiMessage, MetaMessage};
use midly::num::{u4, u7, u15, u24, u28};
use crate::strum::{NoteEvent, EventKind, sort_events};
use anyhow::Result;

/// MIDI file resolution; fine enough to keep strum timing intact.
const TICKS_PER_BEAT: u16 = 960;

/// Convert bpm to microseconds per beat (quarter note).
fn bpm_to_us_per_beat(bpm: f64) -> u24 {
    u24::from((60_000_000.0 / bpm).round() as u32)
}

/// Write timed note events (milliseconds) to a single-track MIDI file,
/// optionally selecting a General MIDI program (instrument) first.
pub fn save_to_midi_file(bpm: f64, events: &[NoteEvent], path: String) -> Result<()> {
    save_to_midi_file_with_program(bpm, events, None, path)
}

pub fn save_to_midi_file_with_program(bpm: f64, events: &[NoteEvent], program: Option<u8>, path: String) -> Result<()> {
    let channel = u4::new(0);
    let off_velocity = u7::from(64);
    let mut track: Vec<TrackEvent> = vec![];

    track.push(TrackEvent {
        delta: u28::from(0),
        kind: TrackEventKind::Meta(MetaMessage::Tempo(bpm_to_us_per_beat(bpm)))
    });
    track.push(TrackEvent {
        delta: u28::from(0),
        kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Dust Chords"))
    });

    if let Some(program) = program {
        track.push(TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Midi { channel, message: MidiMessage::ProgramChange { program: u7::from(program) } },
        });
    }

    let mut events = events.to_vec();
    sort_events(&mut events);

    let ms_per_tick = 60_000.0 / bpm / TICKS_PER_BEAT as f64;
    let mut last_tick: u32 = 0;
    for ev in &events {
        let tick = (ev.at_ms.max(0.0) / ms_per_tick).round() as u32;
        let delta = u28::from(tick.saturating_sub(last_tick));
        last_tick = tick.max(last_tick);
        let message = match ev.kind {
            EventKind::On(vel) => MidiMessage::NoteOn { key: u7::from(ev.note), vel: u7::from(vel) },
            EventKind::Off => MidiMessage::NoteOff { key: u7::from(ev.note), vel: off_velocity },
        };
        track.push(TrackEvent {
            delta,
            kind: TrackEventKind::Midi { channel, message },
        });
    }

    track.push(TrackEvent {
        delta: u28::from(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack)
    });
    let smf = Smf {
        header: Header {
            format: Format::SingleTrack,
            timing: Timing::Metrical(u15::from(TICKS_PER_BEAT))
        },
        tracks: vec![track],
    };
    smf.save(path)?;
    Ok(())
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_bpm_to_us_per_beat() {
        assert_eq!(bpm_to_us_per_beat(60.0), 1_000_000);
        assert_eq!(bpm_to_us_per_beat(120.0), 500_000);
        assert_eq!(bpm_to_us_per_beat(150.0), 400_000);
    }

    #[test]
    fn test_round_trip() {
        let events = vec![
            NoteEvent::on(0.0, 60, 100),
            NoteEvent::on(10.0, 64, 90),
            NoteEvent::off(490.0, 60),
            NoteEvent::off(495.0, 64),
            NoteEvent::on(500.0, 67, 80),
            NoteEvent::off(1000.0, 67),
        ];
        let dir = std::env::temp_dir().join(format!("dust-test-{}.mid", std::process::id()));
        let path = dir.to_string_lossy().to_string();
        save_to_midi_file(120.0, &events, path.clone()).unwrap();
        let data = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let smf = Smf::parse(&data).unwrap();
        let mut t = 0u32;
        let mut got = vec![];
        for ev in &smf.tracks[0] {
            t += ev.delta.as_int();
            if let TrackEventKind::Midi { message, .. } = ev.kind {
                let ms = t as f64 * 500.0 / TICKS_PER_BEAT as f64;
                got.push((ms.round() as u32, message));
            }
        }
        assert_eq!(got.len(), 6);
        assert_eq!(got[0].0, 0);
        assert_eq!(got[1].0, 10);
        assert_eq!(got[4].0, 500);
        assert_eq!(got[5].0, 1000);
        assert!(matches!(got[4].1, MidiMessage::NoteOn { vel, .. } if vel == 80));
    }
}
