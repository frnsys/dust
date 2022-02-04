mod app;
mod core;
mod file;
mod midi;
mod progression;

use clap::{Parser, ValueHint};
use std::{fs::File, path::{Path, PathBuf}, env};
use std::{io, io::BufReader};
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
    #[clap(short, long, value_hint = ValueHint::FilePath)]
    patterns: Option<PathBuf>,

    #[clap(short, long, default_value = "/tmp/", value_hint = ValueHint::DirPath)]
    save_dir: String,
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
    let mut template: ProgressionTemplate = serde_yaml::from_reader(reader).expect("error while reading yaml");
    template.update_transitions();

    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(template, args.save_dir);
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
