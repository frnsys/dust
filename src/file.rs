use midly::{
    Smf, Header, Format, Timing,
    TrackEvent, TrackEventKind,
    MidiMessage, MetaMessage};
use midly::num::{u4, u7, u15, u24, u28};
use crate::core::Chord;
use anyhow::Result;

/// Convert bpm to ms/beat (ms/quarter note)
/// Reference point: 60bpm is 1000ms/beat
fn bpm_to_ms_per_beat(bpm: usize) -> u24 {
    u24::from(60000/bpm as u32)
}

pub fn save_to_midi_file(tempo: usize, ticks_per_beat: usize, progression: &Vec<Option<Chord>>, path: String) -> Result<()> {
    let channel = u4::new(0);
    let velocity = u7::from(64);
    let mut track: Vec<TrackEvent> = vec![];

    // Delta times are in ticks
    let start = u28::from(0);
    let same_time = u28::from(0);

    // Convert from bpm to ms/beat
    let tempo = bpm_to_ms_per_beat(tempo);

    // A beat is a quarter note
    let ticks_per_beat = u15::from(ticks_per_beat as u16);

    // Prepare meta messages
    // Default MIDI time is 4/4 so we exclude that MetaMessage
    track.push(TrackEvent {
        delta: start,
        kind: TrackEventKind::Meta(MetaMessage::Tempo(tempo))
    });
    track.push(TrackEvent {
        delta: start,
        kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Dust Chords"))
    });

    // Add the chords
    let mut pause = 0;
    for tick in progression {
        if let Some(chord) = tick {
            // MIDI note values map A0 to 21.
            // We set A0 to 0 semitones; this our starting point is 0 semitones = MIDI note 21.
            let notes: Vec<u8> = chord.notes().iter().map(|note| (note.semitones + 21) as u8).collect();
            for (i, note) in notes.iter().enumerate() {
                let delta = if i == 0  {
                    u28::from(pause)
                } else {
                    same_time
                };
                track.push(TrackEvent {
                    delta,
                    kind: TrackEventKind::Midi {
                        channel,
                        message: MidiMessage::NoteOn {
                            key: u7::from(*note),
                            vel: velocity
                        }
                    }
                });
            }
            for (i, note) in notes.iter().enumerate() {
                let delta = if i == 0  {
                    u28::from(1)
                } else {
                    same_time
                };
                track.push(TrackEvent {
                    delta,
                    kind: TrackEventKind::Midi {
                        channel,
                        message: MidiMessage::NoteOff {
                            key: u7::from(*note),
                            vel: velocity
                        }
                    }
                });
            }
            pause = 0;
        } else {
            pause += 1;
        }
    }

    track.push(TrackEvent {
        delta: start,
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack)
    });
    let smf = Smf {
        header: Header {
            format: Format::SingleTrack,
            timing: Timing::Metrical(ticks_per_beat)
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
    fn test_bpm_to_ms_per_beat() {
        let ms_per_beat = bpm_to_ms_per_beat(60);
        assert_eq!(ms_per_beat, 1000);

        let ms_per_beat = bpm_to_ms_per_beat(120);
        assert_eq!(ms_per_beat, 500);

        let ms_per_beat = bpm_to_ms_per_beat(150);
        assert_eq!(ms_per_beat, 400);
    }
}
