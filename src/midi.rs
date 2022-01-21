use anyhow::Result;
use thiserror::Error;
use crate::chord::Chord;
use midir::{MidiOutput, MidiOutputConnection, InitError, ConnectError};
use std::{thread, sync::{Arc, Mutex}};
use std::{thread::sleep, time::Duration};

const VELOCITY: u8 = 0x64;
const NOTE_ON_MSG: u8 = 0x90;
const NOTE_OFF_MSG: u8 = 0x80;

pub struct MIDI {
    pub name: Option<String>,
    conn: Arc<Mutex<Option<MidiOutputConnection>>>,
}

#[derive(Error, Debug)]
pub enum MIDIError {
    #[error("No active output connection")]
    NotConnected,

    #[error("Invalid port index: {0}")]
    InvalidPort(usize),

    #[error("Couldn't initialize output")]
    InitError(#[from] InitError),

    #[error("Couldn't connect to output port")]
    ConnectionError(#[from] ConnectError<MidiOutput>),
}

impl MIDI {
    pub fn new() -> MIDI {
        MIDI {
            name: None,
            conn: Arc::new(Mutex::new(None)),
        }
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
            let _ = self.conn.clone().lock().unwrap().insert(conn_out);
            self.name = Some(port_names[idx].to_string());
            Ok(())
        }
    }

    pub fn play_chord(&mut self, chord: &Chord, duration: u64) {
        // MIDI note values map A0 to 21.
        // We set A0 to 0 semitones; this our starting point is 0 semitones = MIDI note 21.
        let notes: Vec<u8> = chord.notes().iter().map(|note| (note.semitones + 21) as u8).collect();
        self.play_notes(notes, duration);
    }

    pub fn play_notes(&mut self, notes: Vec<u8>, duration: u64) {
        let conn = self.conn.clone();
        let _handler = thread::spawn(move || {
            {
                let mut conn = conn.lock().unwrap();
                if let Some(ref mut conn) = *conn {
                    for note in &notes {
                        let _ = conn.send(&[NOTE_ON_MSG, *note, VELOCITY]);
                    }
                }
            }
            sleep(Duration::from_millis(duration * 150));
            {
                let mut conn = conn.lock().unwrap();
                if let Some(ref mut conn) = *conn {
                    for note in &notes {
                        let _ = conn.send(&[NOTE_OFF_MSG, *note, VELOCITY]);
                    }
                }
            }
        });
    }

    pub fn play_note(&mut self, note: u8, duration: u64) {
        let conn = self.conn.clone();
        let _handler = thread::spawn(move || {
            {
                let mut conn = conn.lock().unwrap();
                if let Some(ref mut conn) = *conn {
                    let _ = conn.send(&[NOTE_ON_MSG, note, VELOCITY]);
                }
            }
            sleep(Duration::from_millis(duration * 150));
            {
                let mut conn = conn.lock().unwrap();
                if let Some(ref mut conn) = *conn {
                    let _ = conn.send(&[NOTE_OFF_MSG, note, VELOCITY]);
                }
            }
        });
    }

    pub fn close(&mut self) -> Result<()> {
        let conn = self.conn.clone();
        let mut conn = conn.lock().unwrap();
        let conn = conn.take();
        if let Some(conn) = conn {
            conn.close();
        }
        Ok(())
    }
}
