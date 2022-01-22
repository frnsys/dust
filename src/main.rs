mod app;
mod core;
mod progression;
mod audio;
mod midi;

use std::fs::File;
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


fn main() -> Result<()> {
    let file = File::open("patterns.yaml").expect("could not open file");
    let reader = BufReader::new(file);
    let template: ProgressionTemplate = serde_yaml::from_reader(reader).expect("error while reading yaml");

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
