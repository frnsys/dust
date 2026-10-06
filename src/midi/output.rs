use anyhow::Result;
use super::MIDIError;
use crate::core::Chord;
use crate::strum::{NoteEvent, EventKind};
use midir::{MidiOutput, MidiOutputConnection, os::unix::VirtualOutput};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::{Arc, Mutex, Condvar};
use std::thread;
use std::time::{Duration, Instant};

const VELOCITY: u8 = 0x64;
const OFF_VELOCITY: u8 = 0x40;
const NOTE_ON_MSG: u8 = 0x90;
const NOTE_OFF_MSG: u8 = 0x80;

/// How long one unit of `play_chord`'s duration lasts.
const DURATION_UNIT_MS: u64 = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Msg {
    /// `generation` counts note ons scheduled per note. A note off carries
    /// the generation of the note on it belongs to, so that it doesn't cut
    /// short a later re-trigger of the same note.
    On { note: u8, velocity: u8, generation: u64 },
    Off { note: u8, generation: u64 },
}

#[derive(Debug, PartialEq, Eq)]
struct Scheduled {
    at: Instant,
    seq: u64,
    msg: Msg,
}

// Reverse ordering so the BinaryHeap pops the earliest event first.
impl Ord for Scheduled {
    fn cmp(&self, other: &Self) -> Ordering {
        other.at.cmp(&self.at).then_with(|| other.seq.cmp(&self.seq))
    }
}
impl PartialOrd for Scheduled {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Default)]
struct Queue {
    heap: BinaryHeap<Scheduled>,
    seq: u64,
    /// Latest generation scheduled per note.
    scheduled: HashMap<u8, u64>,
    /// Latest generation actually sent per note.
    sent: HashMap<u8, u64>,
    /// Notes that are currently on.
    sounding: HashSet<u8>,
}

type Conn = Arc<Mutex<Option<MidiOutputConnection>>>;

fn send(conn: &Conn, msg: &[u8]) {
    if let Some(conn) = conn.lock().unwrap().as_mut() {
        let _ = conn.send(msg);
    }
}

/// MIDI output with a scheduler: note events are queued with precise
/// times and sent by a background thread.
pub struct MIDIOutput {
    pub name: Option<String>,
    conn: Conn,
    queue: Arc<(Mutex<Queue>, Condvar)>,
}

impl MIDIOutput {
    pub fn new() -> MIDIOutput {
        let conn: Conn = Arc::new(Mutex::new(None));
        let queue = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
        {
            let conn = conn.clone();
            let queue = queue.clone();
            thread::spawn(move || scheduler_loop(conn, queue));
        }
        MIDIOutput { name: None, conn, queue }
    }

    pub fn from_port(port: usize) -> Result<MIDIOutput, MIDIError> {
        let mut m = MIDIOutput::new();
        m.connect_port(port)?;
        Ok(m)
    }

    /// Create a virtual output port that other applications
    /// (e.g. a DAW) can connect to as a MIDI input.
    pub fn from_virtual(name: &str) -> Result<MIDIOutput, MIDIError> {
        let mut m = MIDIOutput::new();
        m.create_virtual(name)?;
        Ok(m)
    }

    fn output(&self) -> Result<MidiOutput, MIDIError> {
        Ok(MidiOutput::new("Dust Output")?)
    }

    pub fn available_ports(&self) -> Result<Vec<String>, MIDIError> {
        let out = self.output()?;
        let out_ports = out.ports();
        Ok(out_ports.iter().map(|p| out.port_name(p).unwrap()).collect())
    }

    pub fn connect_port(&mut self, idx: usize) -> Result<(), MIDIError> {
        let out = self.output()?;
        let out_ports = out.ports();
        if idx >= out_ports.len() {
            Err(MIDIError::InvalidPort(idx))
        } else {
            let port_names = self.available_ports()?;
            let conn_out = out.connect(&out_ports[idx], "dust")?;
            self.all_notes_off();
            let _ = self.conn.lock().unwrap().insert(conn_out);
            self.name = Some(port_names[idx].to_string());
            Ok(())
        }
    }

    pub fn create_virtual(&mut self, name: &str) -> Result<(), MIDIError> {
        let out = self.output()?;
        let conn_out = out.create_virtual(name)?;
        let _ = self.conn.lock().unwrap().insert(conn_out);
        self.name = Some(name.to_string());
        Ok(())
    }

    /// Schedule note events. Times are milliseconds relative to `base`;
    /// anything already in the past is sent as soon as possible.
    pub fn schedule(&self, base: Instant, events: &[NoteEvent]) {
        let now = Instant::now();
        let (queue, cv) = &*self.queue;
        let mut q = queue.lock().unwrap();
        for ev in events {
            let at = if ev.at_ms <= 0.0 {
                base
            } else {
                base + Duration::from_secs_f64(ev.at_ms / 1000.0)
            };
            let at = at.max(now);
            let msg = match ev.kind {
                EventKind::On(velocity) => {
                    let generation = q.scheduled.entry(ev.note).or_insert(0);
                    *generation += 1;
                    Msg::On { note: ev.note, velocity, generation: *generation }
                }
                EventKind::Off => {
                    let generation = q.scheduled.get(&ev.note).copied().unwrap_or(0);
                    Msg::Off { note: ev.note, generation }
                }
            };
            q.seq += 1;
            let seq = q.seq;
            q.heap.push(Scheduled { at, seq, msg });
        }
        cv.notify_one();
    }

    /// Play a block chord now, held for `duration` units.
    pub fn play_chord(&mut self, chord: &Chord, duration: u64) {
        self.play_notes(chord.midi_notes(), duration);
    }

    pub fn play_notes(&mut self, notes: Vec<u8>, duration: u64) {
        let hold = (duration * DURATION_UNIT_MS) as f64;
        let mut events = vec![];
        for note in notes {
            events.push(NoteEvent::on(0.0, note, VELOCITY));
            events.push(NoteEvent::off(hold, note));
        }
        self.schedule(Instant::now(), &events);
    }

    /// Drop everything scheduled and silence every sounding note.
    pub fn all_notes_off(&self) {
        let (queue, _) = &*self.queue;
        let mut q = queue.lock().unwrap();
        q.heap.clear();
        for note in q.sounding.drain() {
            send(&self.conn, &[NOTE_OFF_MSG, note, OFF_VELOCITY]);
        }
    }

    pub fn close(&mut self) -> Result<()> {
        self.all_notes_off();
        Ok(())
    }
}

impl Default for MIDIOutput {
    fn default() -> Self {
        MIDIOutput::new()
    }
}

fn scheduler_loop(conn: Conn, queue: Arc<(Mutex<Queue>, Condvar)>) {
    let (queue, cv) = &*queue;
    let mut q = queue.lock().unwrap();
    loop {
        let now = Instant::now();
        match q.heap.peek() {
            None => {
                q = cv.wait(q).unwrap();
            }
            Some(next) if next.at > now => {
                let wait = next.at - now;
                q = cv.wait_timeout(q, wait).unwrap().0;
            }
            Some(_) => {
                let ev = q.heap.pop().unwrap();
                match ev.msg {
                    Msg::On { note, velocity, generation } => {
                        let sent = q.sent.entry(note).or_insert(0);
                        *sent = (*sent).max(generation);
                        q.sounding.insert(note);
                        send(&conn, &[NOTE_ON_MSG, note, velocity]);
                    }
                    Msg::Off { note, generation } => {
                        // Skip if a newer note on for this note was already
                        // sent: that one owns the note now.
                        let sent = q.sent.get(&note).copied().unwrap_or(0);
                        if sent <= generation {
                            q.sounding.remove(&note);
                            send(&conn, &[NOTE_OFF_MSG, note, OFF_VELOCITY]);
                        }
                    }
                }
            }
        }
    }
}
