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
use tui::{
    Terminal,
    backend::Backend,
    widgets::Paragraph,
    layout::{Rect, Alignment, Constraint, Direction, Layout},
    text::{Span, Spans},
};
use select::Select;
use sequencer::Sequencer;
use performance::Performance;
use crossterm::event::{self, Event, KeyCode};

const TICK_RATE: Duration = Duration::from_millis(100);

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
}

impl<'a> App<'a> {
    pub fn new(template: ProgressionTemplate, midi_in_port: usize, midi_out_port: usize, save_dir: String) -> App<'a> {
        let midi = MIDIOutput::from_port(midi_out_port).unwrap();
        let midi = Arc::new(Mutex::new(midi));
        let mut seq = Sequencer::new(midi.clone(), template, save_dir.clone());
        seq.connect_port(midi_in_port).unwrap();
        App {
            midi: midi.clone(),
            select: None,
            mode: Mode::Performance,
            sequencer: seq,
            performance: Performance::new(midi.clone(), save_dir),
        }
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.midi.lock().unwrap().close()
    }
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|frame| {
            let size = frame.size();
            let rects = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    // Main rect
                    Constraint::Min(6),

                    // Help rect
                    Constraint::Length(1),
                    ].as_ref())
                .split(size);

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
            let help = Paragraph::new(Spans::from(controls))
                .alignment(Alignment::Left);
            frame.render_widget(help, rects[1]);

            match &mut app.select {
                None => {
                    let chunks: Vec<(Paragraph, Rect)> = match app.mode {
                        Mode::Performance => {
                            app.performance.render(rects[0])
                        }
                        Mode::Sequencer => {
                            app.sequencer.render(rects[0])
                        }
                    };
                    for (p, rect) in chunks {
                        frame.render_widget(p, rect);
                    }
                }
                Some(select) => {
                    let height = rects[0].height as usize;
                    frame.render_widget(select.render(height), rects[0]);
                }
            }
        })?;

        if event::poll(TICK_RATE)? {
            if let Event::Key(key) = event::read()? {
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
                        Some(ref mut select) => {
                            let (selected, close) = select.process_input(key)?;
                            if let Some(idx) = selected {
                                app.midi.lock().unwrap().connect_port(idx).unwrap();
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
                                    let ports = app.midi.lock().unwrap().available_ports().unwrap();
                                    app.select = Some(Select {
                                        idx: 0,
                                        choices: ports,
                                    })
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
