mod app;
mod core;
mod file;
mod midi;
mod progression;

use anyhow::Result;
use app::{App, run_app};
use clap::{Parser, ValueHint};
use progression::ProgressionTemplate;
use ratatui::crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    env,
    fs::File,
    path::{Path, PathBuf},
};
use std::{io, io::BufReader};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    patterns: Option<PathBuf>,

    #[arg(short, long, default_value = "/tmp/", value_hint = ValueHint::DirPath)]
    save_dir: String,

    /// Index of an existing MIDI input port to receive clock from.
    /// By default a virtual input port is created instead.
    #[arg(long)]
    midi_in_port: Option<usize>,

    /// Index of an existing MIDI output port to send notes to.
    /// By default a virtual output port is created instead.
    #[arg(long)]
    midi_out_port: Option<usize>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let path = match args.patterns {
        Some(p) => p,
        None => {
            let home = env::var("HOME").unwrap();
            Path::new(&home).join(".config/dust/patterns.yaml")
        }
    };
    let file = File::open(path).expect("could not open file");
    let reader = BufReader::new(file);
    let mut template: ProgressionTemplate =
        serde_yaml::from_reader(reader).expect("error while reading yaml");
    template.update_transitions();

    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(
        template,
        args.midi_in_port,
        args.midi_out_port,
        args.save_dir,
    );
    let res = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}
