use super::{MIDIInput, MIDIError};

// 4/4 time
const QUARTERS_PER_BAR: usize = 4;

// 24 clock events sent per quarter note
// https://en.wikipedia.org/wiki/MIDI_beat_clock
pub const TICKS_PER_QUARTER: usize = 24;
pub const TICKS_PER_BAR: usize = QUARTERS_PER_BAR * TICKS_PER_QUARTER;

/// Tempo assumed until enough clock pulses have arrived to measure it.
const DEFAULT_BPM: f64 = 120.0;

/// Smoothing factor for the tick interval estimate.
const TEMPO_SMOOTHING: f64 = 0.1;

/// Consecutive implausibly long pulse intervals before the estimate
/// is reset to them (so a large tempo drop isn't rejected forever).
const MAX_REJECTED_INTERVALS: usize = 3;

#[derive(Debug, PartialEq)]
pub enum ClockEvent {
    /// A clock pulse. `tick` counts from 0 at the downbeat of each bar
    /// and `ms_per_tick` is the current estimate of the pulse interval.
    Tick { tick: usize, ms_per_tick: f64 },
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

    /// Connect to an existing input port by index.
    pub fn connect_port<F>(&mut self, idx: usize, tick_fn: F) -> Result<(), MIDIError>
        where F: FnMut(ClockEvent) + Send + 'static {
        self.midi_in.connect_port(idx, Self::handler(tick_fn))
    }

    /// Create a virtual input port for receiving clock messages.
    pub fn create_virtual<F>(&mut self, name: &str, tick_fn: F) -> Result<(), MIDIError>
        where F: FnMut(ClockEvent) + Send + 'static {
        self.midi_in.create_virtual(name, Self::handler(tick_fn))
    }

    /// Build the raw MIDI message handler that translates
    /// clock/start/stop messages into `ClockEvent`s.
    fn handler<F>(mut tick_fn: F) -> impl FnMut(u64, &[u8], &mut ()) + Send + 'static
        where F: FnMut(ClockEvent) + Send + 'static {
        // Per the MIDI spec, the first clock pulse after Start is the downbeat.
        let mut tick = 0;
        let mut playing = false;
        let mut last_stamp: Option<u64> = None;
        let mut ms_per_tick = 60_000.0 / DEFAULT_BPM / TICKS_PER_QUARTER as f64;
        let mut rejected = 0;
        move |stamp_us, msg, _| {
            let ev = match msg {
                [248] => {
                    if let Some(last) = last_stamp {
                        let interval = (stamp_us.saturating_sub(last)) as f64 / 1000.0;
                        if interval > 0.0 && interval < ms_per_tick * 4.0 {
                            ms_per_tick += TEMPO_SMOOTHING * (interval - ms_per_tick);
                            rejected = 0;
                        } else if interval > 0.0 {
                            // A single long gap is a stall (e.g. across a
                            // stop/start); several in a row mean the tempo
                            // really dropped a lot, so re-sync to it.
                            rejected += 1;
                            if rejected >= MAX_REJECTED_INTERVALS {
                                ms_per_tick = interval;
                                rejected = 0;
                            }
                        }
                    }
                    last_stamp = Some(stamp_us);
                    if playing {
                        let ev = ClockEvent::Tick { tick, ms_per_tick };
                        tick = (tick + 1) % TICKS_PER_BAR;
                        Some(ev)
                    } else {
                        None
                    }
                },
                [250] => {
                    playing = true;
                    tick = 0;
                    last_stamp = None;
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
        }
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
