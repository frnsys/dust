mod select;
mod sequencer;
mod text_input;
mod chord_select;
mod performance;

use anyhow::Result;
use std::{
    time::Duration,
    sync::{Arc, Mutex},
};
use crate::midi::MIDIOutput;
use crate::progression::ProgressionTemplate;
use crate::strum::StrumLibrary;
use ratatui::{
    Terminal,
    backend::Backend,
    widgets::Paragraph,
    style::{Style, Color},
    layout::{Rect, Alignment, Constraint, Direction, Layout},
    text::{Span, Line},
};
use select::Select;
use sequencer::Sequencer;
use performance::Performance;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

const TICK_RATE: Duration = Duration::from_millis(100);

/// Names of the virtual MIDI ports created by default.
/// The DAW sends clock to the input and receives notes from the output.
const VIRTUAL_OUT_PORT: &str = "Dust Output";
const VIRTUAL_IN_PORT: &str = "Dust Clock";

pub enum Mode {
    Sequencer,
    Performance,
}

pub struct App<'a> {
    mode: Mode,
    midi: Arc<Mutex<MIDIOutput>>,
    sequencer: Sequencer<'a>,
    performance: Performance<'a>,
    select: Option<Select>,
    /// Last status message, e.g. a port connection error.
    message: String,
}

impl<'a> App<'a> {
    /// If a port index is `None`, a virtual port is created instead
    /// of connecting to an existing one.
    pub fn new(template: ProgressionTemplate, strum_library: StrumLibrary, midi_in_port: Option<usize>, midi_out_port: Option<usize>, save_dir: String) -> App<'a> {
        let midi = match midi_out_port {
            Some(idx) => MIDIOutput::from_port(idx),
            None => MIDIOutput::from_virtual(VIRTUAL_OUT_PORT),
        }.unwrap();
        let midi = Arc::new(Mutex::new(midi));
        let mut seq = Sequencer::new(midi.clone(), template.clone(), strum_library.clone(), save_dir.clone());
        match midi_in_port {
            Some(idx) => seq.connect_port(idx),
            None => seq.create_virtual_port(VIRTUAL_IN_PORT),
        }.unwrap();
        App {
            midi: midi.clone(),
            select: None,
            message: String::new(),
            mode: Mode::Performance,
            sequencer: seq,
            performance: Performance::new(midi.clone(), template, strum_library, save_dir),
        }
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.midi.lock().unwrap().close()
    }
}

pub fn run_app<B>(terminal: &mut Terminal<B>, mut app: App) -> Result<()>
where
    B: Backend,
    B::Error: Send + Sync + 'static,
{
    loop {
        terminal.draw(|frame| {
            let size = frame.area();
            let rects = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    // Params rect
                    Constraint::Length(1),

                    // Main rect
                    Constraint::Min(6),

                    // Controls rect
                    Constraint::Length(1),
                    ].as_ref())
                .split(size);

            // Params help bar
            let mut params = vec![];
            match app.mode {
                Mode::Performance => {
                    params.extend(app.performance.params());
                }
                Mode::Sequencer => {
                    params.extend(app.sequencer.params());
                }
            }

            let params_help = Paragraph::new(Line::from(params))
                .alignment(Alignment::Center);
            frame.render_widget(params_help, rects[0]);

            // Controls help bar
            let mut controls = vec![];
            match app.mode {
                Mode::Performance => {
                    controls.extend(app.performance.controls());
                }
                Mode::Sequencer => {
                    controls.extend(app.sequencer.controls());
                }
            }
            controls.push(
                Span::raw(" [M]ode [P]ort [Q]uit"));
            if !app.message.is_empty() {
                controls.push(Span::styled(
                    format!("  {}", app.message),
                    Style::default().fg(Color::Red)));
            }
            let controls_help = Paragraph::new(Line::from(controls))
                .alignment(Alignment::Left);
            frame.render_widget(controls_help, rects[2]);

            match &mut app.select {
                None => {
                    let chunks: Vec<(Paragraph, Rect)> = match app.mode {
                        Mode::Performance => {
                            app.performance.render(rects[1])
                        }
                        Mode::Sequencer => {
                            app.sequencer.render(rects[1])
                        }
                    };
                    for (p, rect) in chunks {
                        frame.render_widget(p, rect);
                    }
                }
                Some(select) => {
                    let height = rects[1].height as usize;
                    frame.render_widget(select.render(height), rects[1]);
                }
            }
        })?;

        if event::poll(TICK_RATE)? {
            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                // Check if one of the modes is capturing all input
                let input_mode = match app.mode {
                    Mode::Performance => {
                        app.performance.capture_input()
                    }
                    Mode::Sequencer => {
                        app.sequencer.capture_input()
                    }
                };

                if input_mode {
                    match app.mode {
                        Mode::Performance => {
                            app.performance.process_input(key)?;
                        }
                        Mode::Sequencer => {
                            app.sequencer.process_input(key)?;
                        }
                    }
                } else {
                    match &mut app.select {
                        // Midi port selection
                        Some(select) => {
                            let (selected, close) = select.process_input(key)?;
                            if let Some(idx) = selected {
                                app.message = match app.midi.lock().unwrap().connect_port(idx) {
                                    Ok(_) => String::new(),
                                    Err(e) => format!("Couldn't connect to port: {}", e),
                                };
                            }
                            if close {
                                app.select = None;
                            }
                        },
                        None => {
                            match key.code {
                                // Quit
                                KeyCode::Char('Q') => {
                                    app.shutdown().unwrap();
                                    return Ok(());
                                },

                                // Switch mode
                                KeyCode::Char('M') => {
                                    app.mode = match app.mode {
                                        Mode::Sequencer => {
                                            Mode::Performance
                                        },
                                        Mode::Performance => {
                                            Mode::Sequencer
                                        },
                                    }
                                },

                                // Change the MIDI output port
                                KeyCode::Char('P') => {
                                    match app.midi.lock().unwrap().available_ports() {
                                        Ok(ports) => app.select = Some(Select::new(ports)),
                                        Err(e) => app.message = format!("Couldn't list ports: {}", e),
                                    }
                                }
                                _ => {
                                    match app.mode {
                                        Mode::Performance => {
                                            app.performance.process_input(key)?;
                                        }
                                        Mode::Sequencer => {
                                            app.sequencer.process_input(key)?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
