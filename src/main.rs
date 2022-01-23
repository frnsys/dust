mod app;
mod core;
mod midi;
mod audio;
mod progression;

use clap::{Parser, ValueHint};
use std::{fs::File, path::PathBuf};
use std::io::BufReader;
use std::io;
use app::{App, run_app};
use anyhow::Result;
use crossterm::{
    execute,
    event::{DisableMouseCapture, EnableMouseCapture},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    Terminal,
    backend::CrosstermBackend,
};
use progression::ProgressionTemplate;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, default_value = "patterns.yaml", value_hint = ValueHint::FilePath)]
    patterns: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let file = File::open(args.patterns).expect("could not open file");
    let reader = BufReader::new(file);
    let mut template: ProgressionTemplate = serde_yaml::from_reader(reader).expect("error while reading yaml");
    template.update_transitions();

    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(template);
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
