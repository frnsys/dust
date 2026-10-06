use super::MIDIError;
use midir::{MidiInput, MidiInputConnection, os::unix::VirtualInput};

pub struct MIDIInput {
    pub name: Option<String>,
    conn: Option<MidiInputConnection<()>>,
}

impl MIDIInput {
    pub fn new() -> MIDIInput {
        MIDIInput {
            name: None,
            conn: None,
        }
    }

    fn input(&self) -> Result<MidiInput, MIDIError> {
        Ok(MidiInput::new("Dust Input")?)
    }

    pub fn available_ports(&self) -> Result<Vec<String>, MIDIError> {
        let inp = self.input()?;
        let inp_ports = inp.ports();
        Ok(inp_ports.iter().map(|p| inp.port_name(p).unwrap()).collect())
    }

    pub fn connect_port<F>(&mut self, idx: usize, callback: F) -> Result<(), MIDIError>
        where F: FnMut(u64, &[u8], &mut ()) + Send + 'static {
        let inp = self.input()?;
        let inp_ports = inp.ports();
        if idx >= inp_ports.len() {
            Err(MIDIError::InvalidPort(idx))
        } else {
            let port_names = self.available_ports()?;
            let conn_out = inp.connect(&inp_ports[idx], "dust", callback, ())?;
            self.conn = Some(conn_out);
            self.name = Some(port_names[idx].to_string());
            Ok(())
        }
    }

    /// Create a virtual input port that other applications
    /// (e.g. a DAW) can send MIDI to.
    pub fn create_virtual<F>(&mut self, name: &str, callback: F) -> Result<(), MIDIError>
        where F: FnMut(u64, &[u8], &mut ()) + Send + 'static {
        let inp = self.input()?;
        let conn_in = inp.create_virtual(name, callback, ())?;
        self.conn = Some(conn_in);
        self.name = Some(name.to_string());
        Ok(())
    }

    pub fn close(&mut self) {
        if let Some(conn) = self.conn.take() {
            conn.close();
        }
    }
}
