mod select;
mod sequencer;
mod text_input;
mod chord_select;

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
use crossterm::event::{self, Event, KeyCode};

const TICK_RATE: Duration = Duration::from_millis(200);

enum Mode {
    Sequencer,
    Freestyle,
}

pub struct App<'a> {
    mode: Mode,
    midi: Arc<RefCell<MIDI>>,
    sequencer: Sequencer<'a>,
    select: Option<Select>,
}

impl<'a> App<'a> {
    pub fn new(template: ProgressionTemplate, save_dir: String) -> App<'a> {
        let midi = Arc::new(RefCell::new(MIDI::new()));
        App {
            midi: midi.clone(),
            select: None,
            mode: Mode::Sequencer,
            sequencer: Sequencer::new(midi.clone(), template, save_dir),
        }
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

            // Controls help bar
            let mut controls = vec![];
            match app.mode {
                Mode::Freestyle => {
                    // TODO
                }
                Mode::Sequencer => {
                    controls.extend(app.sequencer.controls());
                }
            }
            controls.push(
                Span::raw(" [P]ort [Q]uit"));

            let help = Paragraph::new(Spans::from(controls))
                .alignment(Alignment::Left);
            frame.render_widget(help, rects[1]);

            match &mut app.select {
                None => {
                    let chunks: Vec<(Paragraph, Rect)> = match app.mode {
                        Mode::Freestyle => {
                            // TODO
                            vec![]
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
                    Mode::Freestyle => {
                        // TODO
                        false
                    }
                    Mode::Sequencer => {
                        app.sequencer.capture_input()
                    }
                };

                if input_mode {
                    match app.mode {
                        Mode::Freestyle => {
                            // TODO
                        }
                        Mode::Sequencer => {
                            app.sequencer.process_input(key.code)?;
                        }
                    }
                } else {
                    match &mut app.select {
                        // Midi port selection
                        Some(ref mut select) => {
                            let (selected, close) = select.process_input(key.code)?;
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
                                        Mode::Freestyle => {
                                            // TODO
                                        }
                                        Mode::Sequencer => {
                                            app.sequencer.process_input(key.code)?;
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
