use super::{MIDIInput, MIDIError};

// 4/4 time
const QUARTERS_PER_BAR: usize = 4;

// 24 clock events sent per quarter note
// https://en.wikipedia.org/wiki/MIDI_beat_clock
const TICKS_PER_QUARTER: usize = 24;

#[derive(Debug, PartialEq, Eq)]
pub enum ClockEvent {
    Tick(usize),
    Start,
    Stop,
}

pub struct MIDIClock {
    midi_in: MIDIInput,
}

impl MIDIClock {
    pub fn new() -> MIDIClock {
        MIDIClock {
            midi_in: MIDIInput::new(),
        }
    }

    pub fn connect_port<F>(&mut self, idx: usize, mut tick_fn: F) -> Result<(), MIDIError>
        where F: FnMut(ClockEvent) + Send + 'static {
        let mut tick = 0;
        let mut playing = false;
        self.midi_in.connect_port(idx, move |_, msg, _| {
            let ev = match msg {
                [248] => {
                    if playing {
                        tick += 1;
                        if tick >= QUARTERS_PER_BAR * TICKS_PER_QUARTER {
                            tick = 0;
                        }
                        Some(ClockEvent::Tick(tick))
                    } else {
                        None
                    }
                },
                [250] => {
                    playing = true;
                    Some(ClockEvent::Start)
                },
                [252] => {
                    playing = false;
                    Some(ClockEvent::Stop)
                },
                _ => None,
            };
            if let Some(ev) = ev {
                tick_fn(ev);
            }
        })
    }

    pub fn close(&mut self) {
        self.midi_in.close();
    }
}

impl Default for MIDIClock {
    fn default() -> Self {
        MIDIClock::new()
    }
}
