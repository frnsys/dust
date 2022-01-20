use kira::{
    Tempo,
    Duration,
    arrangement::{Arrangement, ArrangementSettings, SoundClip, handle::ArrangementHandle},
	sound::{SoundSettings, handle::SoundHandle},
	manager::{AudioManager, AudioManagerSettings},
	instance::InstanceSettings,
    sequence::{Sequence, SequenceSettings, SequenceInstanceSettings, handle::SequenceInstanceHandle},
    metronome::{MetronomeSettings, handle::MetronomeHandle},
};
use std::collections::HashMap;
use crate::{chord::Chord, note::Note};
use anyhow::Result;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Event {
	Beat,
}

pub struct Audio {
    manager: AudioManager,

    /// Cache sounds
    sounds: HashMap<String, SoundHandle>,

    /// Currently playing progression
    progression: Option<Progression>,
}

struct Progression {
    metronome: MetronomeHandle,
    sequence: SequenceInstanceHandle<Event>,
    chords: Vec<ArrangementHandle>,
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

    pub fn play_progression(&mut self, tempo: f64, chords: &Vec<Chord>) -> Result<()> {
        let tempo = Tempo(tempo);
        let mut metronome = self.manager.add_metronome(MetronomeSettings::new().tempo(tempo))?;
        let chord_handles: Vec<ArrangementHandle> = chords.iter()
            .map(|chord| self.build_chord(chord)).collect::<Result<Vec<_>, _>>()?;

        let sequence_handle = self.manager.start_sequence::<Event>(
            {
                let mut sequence = Sequence::new(SequenceSettings::default());
                sequence.start_loop();
                for chord_handle in &chord_handles {
                    sequence.emit(Event::Beat);
                    sequence.play(chord_handle, InstanceSettings::default());
                    sequence.wait(Duration::Beats(1.0));
                }
                sequence
            },
            SequenceInstanceSettings::new().metronome(&metronome),
        )?;
        metronome.start()?;
        self.progression = Some(Progression {
            metronome,
            sequence: sequence_handle,
            chords: chord_handles,
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
        if !self.sounds.contains_key(&fname) {
            let sound = self.manager.load_sound(fname.clone(), SoundSettings::default())?;
            self.sounds.insert(fname.clone(), sound);
        }
        // OK to unwrap because we check the key's existence
        Ok(self.sounds.get(&fname).unwrap().clone())
    }

    pub fn stop_progression(&mut self) -> Result<()> {
        if let Some(prog) = &mut self.progression {
            prog.sequence.stop()?;
            self.manager.remove_metronome(&prog.metronome)?;
            for chord in &prog.chords {
                self.manager.remove_arrangement(chord)?;
            }
        }
        Ok(())
    }
}
