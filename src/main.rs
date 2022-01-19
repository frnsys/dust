mod key;
mod note;
mod chord;
mod interval;
mod progression;
mod audio;

use clap::Parser;
use anyhow::Result;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use audio::Audio;
use key::{Key, Mode};
use progression::{ChordSpec, Quality};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Tempo
    #[clap(short, long, default_value_t = 120)]
    tempo: u8,

    /// Bars
    #[clap(short, long, default_value_t = 8)]
    bars: usize,

    /// Mode
    #[clap(arg_enum, default_value_t = Mode::Major)]
    mode: Mode,

    /// Root
    #[clap(short, long, default_value = "C3")]
    root: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Exit on ctrl+c
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    let mut audio = Audio::new()?;
    let key = Key {
        root: args.root.try_into()?,
        mode: args.mode,
    };
    let spec = ChordSpec::new(0, Quality::Major);
    let progression = spec.gen_progression(args.bars);
    let progression_in_key = progression.iter().map(|cs| cs.chord_for_key(&key)).collect();
    let mut sequence = audio.play_progression(args.tempo as f64, &progression_in_key)?;

    println!("{}", progression.iter().map(|cs| cs.to_string()).collect::<Vec<String>>().join(" -> "));
    while running.load(Ordering::SeqCst) {
        if let Some(event) = sequence.pop_event()? {
            // println!("{:?}", event);
        }
    }
    Ok(())
}
