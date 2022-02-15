use thiserror::Error;
use midir::{MidiOutput, MidiInput, InitError, ConnectError};

#[derive(Error, Debug)]
pub enum MIDIError {
    #[error("No active connection")]
    NotConnected,

    #[error("Invalid port index: {0}")]
    InvalidPort(usize),

    #[error("Couldn't initialize")]
    InitError(#[from] InitError),

    #[error("Couldn't connect to output port")]
    OutputConnect(#[from] ConnectError<MidiOutput>),

    #[error("Couldn't connect to input port")]
    InputConnect(#[from] ConnectError<MidiInput>),
}
