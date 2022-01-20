use crossterm::event::{self, Event, KeyCode};
use tui::{
    Terminal,
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Style, Modifier, Color},
    text::{Span, Spans},
    widgets::{Block, Paragraph},
};
use anyhow::Result;
use crate::key::{Key, Mode};
use crate::note::Note;
use crate::audio::Audio;
use crate::progression::{ChordSpec, Quality};

enum InputMode {
    Normal,
    Editing,
}

enum InputTarget {
    Root,
    Tempo,
    Bars
}

pub struct App<'a> {
    /// Text input handling
    input: String,
    input_mode: InputMode,
    input_target: InputTarget,

    /// Last status message
    message: &'a str,

    /// Params
    tempo: usize,
    bars: usize,
    mode: Mode,
    root: Note,

    audio: Audio,
    progression: Vec<ChordSpec>,
}

impl<'a> App<'a> {
    fn gen_progression(&mut self) -> Result<()> {
        self.audio.stop_progression()?;
        let key = Key {
            root: self.root,
            mode: self.mode,
        };
        let start_chord = ChordSpec::new(0, Quality::Major);
        self.progression = start_chord.gen_progression(self.bars);
        let progression_in_key = self.progression.iter().map(|cs| cs.chord_for_key(&key)).collect();
        self.audio.play_progression(self.tempo as f64, &progression_in_key)?;
        Ok(())
    }
}

impl<'a> Default for App<'a> {
    fn default() -> App<'a> {
        let mut app = App {
            audio: Audio::new().unwrap(),

            input: String::new(),
            input_mode: InputMode::Normal,
            input_target: InputTarget::Tempo,
            message: "",

            tempo: 120,
            bars: 8,
            mode: Mode::Major,
            root: Note {
                semitones: 27
            }, // C3
            progression: vec![]
        };
        app.gen_progression().unwrap();
        app
    }
}


fn render_progression<'a>(progression: &Vec<ChordSpec>) -> Paragraph<'a> {
    let prog_desc = progression.iter().map(|cs| cs.to_string()).collect::<Vec<String>>().join(" -> ");
    Paragraph::new(vec![
        Spans::from(vec![Span::raw(prog_desc)]),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .style(Style::default())
    )
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|rect| {
            let size = rect.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                        // To vertically center the text in the progression chunk
                        Constraint::Length(size.height/2 - 3),

                        // Progression chunk
                        Constraint::Min(2),

                        // Status bar chunk
                        Constraint::Length(1),
                    ].as_ref())
                .split(size);

            let param_style = Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD);
            let status = Spans::from(vec![
                Span::raw("[r]oot:"),
                Span::styled(app.root.to_string(), param_style),
                Span::raw(" [b]ars:"),
                Span::styled(app.bars.to_string(), param_style),
                Span::raw(" [m]ode:"),
                Span::styled(app.mode.to_string(), param_style),
                Span::raw(" [t]empo:"),
                Span::styled(app.tempo.to_string(), param_style),
                Span::raw(" [R]roll [q]uit"),
            ]);
            let help = Paragraph::new(status)
                .style(Style::default())
                .alignment(Alignment::Left)
                .block(
                    Block::default()
                );

            let messages = match app.input_mode {
                InputMode::Normal =>  {
                    Paragraph::new(app.message)
                        .style(Style::default())
                        .alignment(Alignment::Right)
                        .block(
                            Block::default()
                        )
                },
                InputMode::Editing => {
                    let label = match app.input_target {
                        InputTarget::Root => "Root: ",
                        InputTarget::Tempo => "Tempo: ",
                        InputTarget::Bars => "Bars: ",
                    };
                    let spans = Spans::from(vec![
                        Span::raw(label),
                        Span::styled(&app.input, Style::default().fg(Color::LightBlue))
                    ]);
                    Paragraph::new(spans)
                        .style(Style::default())
                        .alignment(Alignment::Right)
                        .block(
                            Block::default()
                        )

                }
            };

            let status_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50)].as_ref(),
                )
                .split(chunks[2]);
            rect.render_widget(help, status_chunks[0]);
            rect.render_widget(messages, status_chunks[1]);
            rect.render_widget(render_progression(&app.progression), chunks[1]);
        })?;

        if let Event::Key(key) = event::read()? {
            app.message = "";
            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('t') => {
                        app.input = app.tempo.to_string();
                        app.input_mode = InputMode::Editing;
                        app.input_target = InputTarget::Tempo;
                    }
                    KeyCode::Char('b') => {
                        app.input = app.bars.to_string();
                        app.input_mode = InputMode::Editing;
                        app.input_target = InputTarget::Bars;
                    }
                    KeyCode::Char('r') => {
                        app.input = app.root.to_string();
                        app.input_mode = InputMode::Editing;
                        app.input_target = InputTarget::Root;
                    }
                    KeyCode::Char('m') => {
                        app.mode = match app.mode {
                            Mode::Major => Mode::Minor,
                            Mode::Minor => Mode::Major,
                        };
                        app.gen_progression()?;
                    }
                    KeyCode::Char('R') => {
                        app.gen_progression()?;
                    }
                    KeyCode::Char('q') => {
                        return Ok(());
                    }
                    _ => {}
                },
                InputMode::Editing => match key.code {
                    KeyCode::Enter => {
                        let input = app.input.drain(..)
                            .collect::<String>();
                        if input.len() > 0 {
                            match app.input_target {
                                InputTarget::Root => {
                                    app.root = match input.try_into() {
                                        Ok(note) => {
                                            note
                                        }
                                        Err(_) => {
                                            app.message = "Invalid root note";
                                            app.root
                                        }
                                    };
                                }
                                InputTarget::Tempo => {
                                    app.tempo = input.parse::<usize>()?;
                                }
                                InputTarget::Bars => {
                                    app.bars = input.parse::<usize>()?;
                                }
                            }
                            app.gen_progression()?;
                        }
                        app.input_mode = InputMode::Normal;
                    }
                    KeyCode::Char(c) => {
                        match app.input_target {
                            InputTarget::Root => {
                                if c.is_alphanumeric() {
                                    app.input.push(c);
                                }
                            },
                            _ => {
                                if c.is_numeric() {
                                    app.input.push(c);
                                }
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                    }
                    _ => {}
                },
            }
        }
    }
}
