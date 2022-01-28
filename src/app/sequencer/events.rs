use kira::{
    Tempo,
    Duration,
    sound::SoundSettings,
	instance::InstanceSettings,
	manager::{AudioManager, AudioManagerSettings},
    metronome::{MetronomeSettings, handle::MetronomeHandle},
    sequence::{Sequence, SequenceSettings, SequenceInstanceSettings, handle::SequenceInstanceHandle},
};
use anyhow::Result;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Event {}

pub struct EventEmitter {
    manager: AudioManager,

    /// Currently playing progression
    metronome: Option<Metronome>,
}

struct Metronome {
    metronome: MetronomeHandle,
    paused: bool,

    // Sequence for metronome ticks
    tick: SequenceInstanceHandle<Event>,
    muted: bool,
}

impl Metronome {
    fn mute(&mut self) -> Result<()> {
        self.tick.mute()?;
        self.muted = true;
        Ok(())
    }

    fn unmute(&mut self) -> Result<()> {
        self.tick.unmute()?;
        self.muted = false;
        Ok(())
    }

    fn toggle(&mut self) -> Result<()> {
        if self.muted {
            self.unmute()?;
        } else {
            self.mute()?;
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.paused = true;
        self.tick.pause()?;
        self.metronome.pause()?;
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
        self.paused = false;
        self.tick.resume()?;
        self.metronome.start()?;
        Ok(())
    }
}

impl EventEmitter {
    pub fn new() -> Result<EventEmitter> {
        let manager = AudioManager::new(AudioManagerSettings::default())?;
        Ok(EventEmitter {
            manager,
            metronome: None,
        })
    }

    pub fn start(&mut self, tempo: f64, time_unit: f64) -> Result<()> {
        let tempo = Tempo(tempo);

        // Set the metronome to emit events for each time unit
        let ticks = vec![time_unit];
        let mut metronome = self.manager.add_metronome(
            MetronomeSettings::new().tempo(tempo).interval_events_to_emit(ticks))?;

        // The sequence that actually emits metronome tick sounds
        let metronome_sound = self.manager.load_sound("samples/metronome.wav", SoundSettings::default())?;
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

        metronome.start()?;
        self.metronome = Some(Metronome {
            metronome,
            muted: false,
            paused: false,
            tick: tick_sequence_handle,
        });
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(met) = &mut self.metronome {
            met.tick.stop()?;
            self.manager.remove_metronome(&met.metronome)?;
        }
        Ok(())
    }

    pub fn is_paused(&self) -> bool {
        if let Some(ref met) = self.metronome {
            met.paused
        } else {
            true
        }
    }

    pub fn pause(&mut self) -> Result<()> {
        if let Some(ref mut met) = self.metronome {
            met.pause()?;
        }
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
        if let Some(ref mut met) = self.metronome {
            met.resume()?;
        }
        Ok(())
    }

    pub fn toggle_tick(&mut self) -> Result<()> {
        if let Some(ref mut met) = self.metronome {
            met.toggle()?;
        }
        Ok(())
    }

    pub fn pop_event(&mut self) -> Result<Option<f64>> {
        if let Some(ref mut met) = self.metronome {
            Ok(met.metronome.pop_event()?)
        } else {
            Ok(None)
        }
    }
}
