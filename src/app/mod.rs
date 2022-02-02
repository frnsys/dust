mod select;
mod sequencer;
mod text_input;
mod chord_select;
mod performance;

use anyhow::Result;
use crate::midi::MIDI;
use std::{
    sync::Arc,
    cell::RefCell,
    time::{Duration, Instant},
};
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

const TICK_RATE: Duration = Duration::from_millis(200);

pub enum Mode {
    Sequencer,
    Performance,
}

pub struct App<'a> {
    mode: Mode,
    midi: Arc<RefCell<MIDI>>,
    sequencer: Sequencer<'a>,
    performance: Performance<'a>,
    select: Option<Select>,

    // Cache unchanging elements
    help: Paragraph<'a>,
}

impl<'a> App<'a> {
    pub fn new(template: ProgressionTemplate, save_dir: String) -> App<'a> {
        let midi = Arc::new(RefCell::new(MIDI::new()));
        let mut app = App {
            midi: midi.clone(),
            select: None,
            mode: Mode::Performance,
            sequencer: Sequencer::new(midi.clone(), template, save_dir.clone()),
            performance: Performance::new(midi.clone(), save_dir),
            help: Paragraph::new(Spans::from("")),
        };
        app.help = app.render_help();
        app
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.help = self.render_help();
    }

    fn render_help(&mut self) -> Paragraph<'a> {
        // Controls help bar
        let mut controls = vec![];
        match self.mode {
            Mode::Performance => {
                controls.extend(self.performance.controls());
            }
            Mode::Sequencer => {
                controls.extend(self.sequencer.controls());
            }
        }
        controls.push(
            Span::raw(" [M]ode [P]ort [Q]uit"));

        Paragraph::new(Spans::from(controls))
            .alignment(Alignment::Left)
    }
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    let mut last_tick = Instant::now();
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

            frame.render_widget(app.help.clone(), rects[1]);

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

        let timeout = TICK_RATE
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)? {
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
                                app.midi.borrow_mut().connect_port(idx).unwrap();
                            }
                            if close {
                                app.select = None;
                            }
                        },
                        None => {
                            match key.code {
                                // Quit
                                KeyCode::Char('Q') => {
                                    return Ok(());
                                },

                                // Switch mode
                                KeyCode::Char('M') => {
                                    match app.mode {
                                        Mode::Sequencer => {
                                            app.sequencer.pause()?;
                                            app.set_mode(Mode::Performance);
                                        },
                                        Mode::Performance => {
                                            app.sequencer.resume()?;
                                            app.set_mode(Mode::Sequencer);
                                        },
                                    }
                                },

                                // Change the MIDI output port
                                KeyCode::Char('P') => {
                                    let ports = app.midi.borrow().available_ports().unwrap();
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
            if last_tick.elapsed() >= TICK_RATE {
                last_tick = Instant::now();
            }
        }
    }
}
