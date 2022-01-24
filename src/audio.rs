use kira::{
    Tempo,
    Duration,
    arrangement::{Arrangement, ArrangementSettings, SoundClip, handle::ArrangementHandle},
	sound::{SoundSettings, handle::SoundHandle},
	manager::{AudioManager, AudioManagerSettings},
	instance::InstanceSettings,
    sequence::{Sequence, SequenceSettings, SequenceInstanceSettings, SequenceInstanceState, handle::SequenceInstanceHandle},
    metronome::{MetronomeSettings, handle::MetronomeHandle},
};
use std::collections::HashMap;
use crate::core::{Chord, Note};
use anyhow::Result;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Event {
	Chord(usize),
}

pub struct Audio {
    manager: AudioManager,

    /// Cache sounds
    sounds: HashMap<String, SoundHandle>,

    /// Currently playing progression
    pub progression: Option<AudioProgression>,
}

pub struct AudioProgression {
    pub metronome: MetronomeHandle,
    chords: Vec<Option<ArrangementHandle>>,
    sequence: SequenceInstanceHandle<Event>,

    // Sequence for metronome ticks
    tick_sequence: SequenceInstanceHandle<Event>,
    tick_muted: bool,

    // A silent sequence that doesn't emit sounds,
    // for driving MIDI outpuut
    pub event_sequence: SequenceInstanceHandle<Event>,
}

impl AudioProgression {
    pub fn mute_tick(&mut self) -> Result<()> {
        self.tick_sequence.mute()?;
        self.tick_muted = true;
        Ok(())
    }

    pub fn unmute_tick(&mut self) -> Result<()> {
        self.tick_sequence.unmute()?;
        self.tick_muted = false;
        Ok(())
    }

    pub fn toggle_tick(&mut self) -> Result<()> {
        if self.tick_muted {
            self.unmute_tick()?;
        } else {
            self.mute_tick()?;
        }
        Ok(())
    }

    pub fn mute(&mut self) -> Result<()> {
        self.sequence.mute()?;
        self.tick_sequence.mute()?;
        Ok(())
    }

    pub fn unmute(&mut self) -> Result<()> {
        self.sequence.unmute()?;
        if !self.tick_muted {
            self.tick_sequence.unmute()?;
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        self.sequence.pause()?;
        self.event_sequence.pause()?;
        self.tick_sequence.pause()?;
        self.metronome.pause()?;
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
        self.sequence.resume()?;
        self.event_sequence.resume()?;
        self.tick_sequence.resume()?;
        self.metronome.start()?;
        Ok(())
    }
}

impl Audio {
    pub fn new() -> Result<Audio> {
        let manager = AudioManager::new(AudioManagerSettings::default())?;
        Ok(Audio {
            manager,
            progression: None,
            sounds: HashMap::default(),
        })
    }

    pub fn play_progression(&mut self, tempo: f64, time_unit: f64, seq: &[Option<Chord>]) -> Result<()> {
        let tempo = Tempo(tempo);

        // Set the metronome to emit events for each time unit
        let ticks = vec![time_unit];
        let mut metronome = self.manager.add_metronome(
            MetronomeSettings::new().tempo(tempo).interval_events_to_emit(ticks))?;

        let chord_handles: Vec<Option<ArrangementHandle>> = seq.iter()
            .map(|cs| {
                match cs {
                    Some(cs) => Some(self.build_chord(cs).unwrap()),
                    None => None
                }
            }).collect();

        // The sequence that actually emits chord sounds
        let sequence_handle = self.manager.start_sequence::<Event>(
            {
                let mut sequence = Sequence::new(SequenceSettings::default());
                sequence.start_loop();
                for ch in &chord_handles {
                    match ch {
                        Some(ch) => {
                            sequence.play(ch, InstanceSettings::default());
                        }
                        _ => {}
                    }
                    sequence.wait(Duration::Beats(time_unit));
                }
                sequence
            },
            SequenceInstanceSettings::new().metronome(&metronome),
        )?;

        // The sequence that actually emits metronome tick sounds
        let metronome_sound = self.load_sound("samples/metronome.wav".to_string()).unwrap();
        let tick_sequence_handle = self.manager.start_sequence::<Event>(
            {
                let mut sequence = Sequence::new(SequenceSettings::default());
                sequence.start_loop();
                sequence.play(&metronome_sound, InstanceSettings::default());
                sequence.wait(Duration::Beats(1.));
                sequence
            },
            SequenceInstanceSettings::new().metronome(&metronome),
        )?;

        // A separate event sequence so we can continue
        // sending events while audio is muted (for driving MIDI output)
        let event_sequence_handle = self.manager.start_sequence::<Event>(
            {
                let mut sequence = Sequence::new(SequenceSettings::default());
                sequence.start_loop();
                for (i, ch) in chord_handles.iter().enumerate() {
                    match ch {
                        Some(_) => {
                            sequence.emit(Event::Chord(i));
                        }
                        _ => {}
                    }
                    sequence.wait(Duration::Beats(time_unit));
                }
                sequence
            },
            SequenceInstanceSettings::new().metronome(&metronome),
        )?;

        metronome.start()?;
        self.progression = Some(AudioProgression {
            metronome,
            tick_muted: false,
            chords: chord_handles,
            sequence: sequence_handle,
            event_sequence: event_sequence_handle,
            tick_sequence: tick_sequence_handle,
        });
        Ok(())
    }

    fn build_chord(&mut self, chord: &Chord) -> Result<ArrangementHandle> {
        let mut arrangement = Arrangement::new(
            ArrangementSettings::new()
        );
        for note in chord.notes() {
            let handle = self.note_sound(&note)?;
            arrangement
                .add_clip(SoundClip::new(&handle, 0.0));
        }
        Ok(self.manager.add_arrangement(arrangement)?)
    }

    fn note_sound(&mut self, note: &Note) -> Result<SoundHandle> {
        let fname = format!("samples/piano/ogg/Piano.ff.{}.ogg", note.to_string());
        self.load_sound(fname)
    }

    fn load_sound(&mut self, path: String) -> Result<SoundHandle> {
        if !self.sounds.contains_key(&path) {
            let sound = self.manager.load_sound(path.clone(), SoundSettings::default())?;
            self.sounds.insert(path.clone(), sound);
        }
        // OK to unwrap because we check the key's existence
        Ok(self.sounds.get(&path).unwrap().clone())
    }

    pub fn stop_progression(&mut self) -> Result<()> {
        if let Some(prog) = &mut self.progression {
            prog.sequence.stop()?;
            self.manager.remove_metronome(&prog.metronome)?;
            for chord in &prog.chords {
                if let Some(ch) = chord {
                    self.manager.remove_arrangement(ch)?;
                }
            }
        }
        Ok(())
    }

    pub fn mute(&mut self) -> Result<()> {
        if let Some(ref mut prog) = self.progression {
            prog.mute()?;
        }
        Ok(())
    }

    pub fn unmute(&mut self) -> Result<()> {
        if let Some(ref mut prog) = self.progression {
            prog.unmute()?;
        }
        Ok(())
    }

    pub fn is_paused(&self) -> bool {
        if let Some(ref prog) = self.progression {
            prog.sequence.state() == SequenceInstanceState::Paused
        } else {
            true
        }
    }

    pub fn pause(&mut self) -> Result<()> {
        if let Some(ref mut prog) = self.progression {
            prog.pause()?;
        }
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
        if let Some(ref mut prog) = self.progression {
            prog.resume()?;
        }
        Ok(())
    }

    pub fn toggle_tick(&mut self) -> Result<()> {
        if let Some(ref mut prog) = self.progression {
            prog.toggle_tick()?;
        }
        Ok(())
    }
}
