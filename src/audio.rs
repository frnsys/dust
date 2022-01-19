use kira::{
    Tempo,
    Duration,
    arrangement::{Arrangement, ArrangementSettings, SoundClip, handle::ArrangementHandle},
	sound::{SoundSettings, handle::SoundHandle},
	manager::{AudioManager, AudioManagerSettings},
	instance::InstanceSettings,
    sequence::{Sequence, SequenceSettings, SequenceInstanceSettings, handle::SequenceInstanceHandle},
    metronome::MetronomeSettings,
};
use crate::{chord::Chord, note::Note};
use anyhow::Result;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Event {
	Beat,
}

pub struct Audio {
    manager: AudioManager,
}

impl Audio {
    pub fn new() -> Result<Audio> {
        let manager = AudioManager::new(AudioManagerSettings::default())?;
        Ok(Audio {
            manager
        })
    }

    pub fn play_progression(&mut self, tempo: f64, chords: &Vec<Chord>) -> Result<SequenceInstanceHandle<Event>> {
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
        Ok(sequence_handle)
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
        Ok(self.manager.load_sound(fname, SoundSettings::default())?)
    }
}
