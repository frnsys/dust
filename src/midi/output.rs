use anyhow::Result;
use super::MIDIError;
use crate::core::Chord;
use midir::{MidiOutput, MidiOutputConnection};
use std::{thread, sync::{Arc, Mutex}};
use std::{thread::sleep, time::Duration};
use std::collections::HashMap;

const VELOCITY: u8 = 0x64;
const NOTE_ON_MSG: u8 = 0x90;
const NOTE_OFF_MSG: u8 = 0x80;

pub struct MIDIOutput {
    pub name: Option<String>,
    conn: Arc<Mutex<Option<MidiOutputConnection>>>,

    // We use this to determine when a note off
    // signal should be sent, to avoid conflicts
    note_owners: Arc<Mutex<HashMap<u8, usize>>>,
}


impl MIDIOutput {
    pub fn new() -> MIDIOutput {
        MIDIOutput {
            name: None,
            conn: Arc::new(Mutex::new(None)),
            note_owners: Arc::new(Mutex::new(HashMap::default())),
        }
    }

    pub fn from_port(port: usize) -> Result<MIDIOutput, MIDIError> {
        let mut m = MIDIOutput::new();
        m.connect_port(port)?;
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

        // When we play a set of notes, we need to track
        // which thread has the right to stop those notes
        // (i.e. send the notes off message).
        // This is to avoid the following scenario:
        // - t=0.0: Thread A plays CEG for 1 second
        // - t=0.5: Thread B plays CEG again
        // - t=1.0: Thread A stops CEG, prematurely ending thread B's CEG by 0.5 seconds
        // Here we assign a number to each thread that plays a given note.
        // Then before that thread stops its notes, it checks to see if it
        // is the owner (has the highest number) of those notes.
        // For simplicity just saying one note (C) but this applies for multiple notes too.
        // - t=0.0: Thread A plays C for 1 second and is assigned #1.
        // - t=0.5: Thread B plays C again and is assigned #2
        // - t=1.0: Thread A wants to stop C, so it compares its number (#1)
        //  against C's current number (#2). Because #1 < #2, thread A doesn't stop C.
        // - t=1.5: Thread B wants to stop C, so it compares its number (#2)
        //  against C's current number (#2). Because #2 = #2, thread A can stop C.
        let mut my_notes: HashMap<u8, usize> = HashMap::default();
        for note in &notes {
            let mut note_owners = self.note_owners.lock().unwrap();
            let n = note_owners.entry(*note).or_insert(0);
            *n += 1;
            my_notes.insert(*note, *n);
        }
        let note_owners = self.note_owners.clone();

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
                    let owners = note_owners.lock().unwrap();
                    for note in &notes {
                        let my_number = my_notes.get(note).unwrap();
                        if my_number >= owners.get(note).unwrap() {
                            let _ = conn.send(&[NOTE_OFF_MSG, *note, VELOCITY]);
                        }
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

        if let Some(ref mut conn) = *conn {
            let note_owners = self.note_owners.lock().unwrap();
            for note in note_owners.keys() {
                let _ = conn.send(&[NOTE_OFF_MSG, *note, VELOCITY]);
            }
        }
        Ok(())
    }
}
