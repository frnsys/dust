use std::fmt;
use crossterm::event::{self, Event, KeyCode};
use::std::time::{Duration, Instant};
use tui::{
    Terminal,
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Style, Modifier, Color},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Borders},
};
use anyhow::Result;
use crate::note::Note;
use crate::key::{Key, Mode};
use crate::chord::{Chord, ChordSpec};
use crate::audio::{Audio, Event as AudioEvent};
use crate::midi::MIDI;
use crate::progression::ProgressionTemplate;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum InputMode {
    Normal,
    Editing,
    Select,
}

enum InputTarget {
    Root,
    Tempo,
    Bars,
    MidiPort,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Output {
    Audio,
    Midi
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Output::Audio => "Audio",
            Output::Midi => "MIDI"
        };
        write!(f, "{}", name)
    }
}

pub struct App<'a> {
    /// Text input handling
    input: String,
    input_mode: InputMode,
    input_target: InputTarget,
    choices: Vec<String>,

    /// Last status message
    message: &'a str,

    /// Params
    tempo: usize,
    bars: usize,
    key: Key,

    // Current chord in the progression
    chord_idx: usize,
    progression: Vec<(ChordSpec, f64)>,
    progression_in_key: Vec<(Chord, f64)>,
    template: ProgressionTemplate,

    // Outputs
    output: Output,
    audio: Audio,
    midi: MIDI,
}

impl<'a> App<'a> {
    pub fn new(template: ProgressionTemplate) -> App<'a> {
        let mut app = App {
            chord_idx: 0,
            output: Output::Audio,
            audio: Audio::new().unwrap(),
            midi: MIDI::new(),

            input: String::new(),
            input_mode: InputMode::Normal,
            input_target: InputTarget::Tempo,
            choices: vec![],
            message: "",

            bars: 8,
            tempo: 100,
            key: Key {
                mode: Mode::Major,
                root: Note {
                    semitones: 27
                }, // C3
            },
            progression: vec![],
            progression_in_key: vec![],
            template,
        };
        app.gen_progression().unwrap();
        app
    }

    /// Updates and plays the current progression with the current key and tempo.
    fn update_progression(&mut self) -> Result<()> {
        self.audio.stop_progression()?;
        self.progression_in_key = self.progression.iter().map(|cs| (cs.0.chord_for_key(&self.key), cs.1)).collect();
        self.audio.play_progression(self.tempo as f64, &self.progression_in_key)?;
        // If output is MIDI, mute the audio.
        // We don't pause it because its events
        // drive the MIDI output.
        match self.output {
            Output::Audio => self.audio.unmute()?,
            Output::Midi => self.audio.mute()?
        }
        Ok(())

    }

    /// Generates and plays a new random progression.
    fn gen_progression(&mut self) -> Result<()> {
        let start_chord: ChordSpec = self.template.rand_chord_for_mode(&self.key.mode);
        self.progression = self.template.gen_progression(&start_chord, self.bars, &self.key.mode);
        self.update_progression()?;
        Ok(())
    }
}

fn render_progression<'a>(progression: &Vec<(ChordSpec, f64)>, key: &Key, idx: usize) -> Paragraph<'a> {
    // The lines that will be rendered.
    let mut lines = vec![];

    // Keep track of the notes of each chord
    // for displaying.
    let mut chord_notes = vec![];

    // Keep track of how many lines are required
    // to display the chord notes underneath.
    // This is just the highest number of notes
    // in a chord in the progression.
    let mut required_lines = 0;

    // The spans for the chord
    let chord_name_spans: Vec<Span> = progression.iter().enumerate().map(|(i, (cs, _))| {
        // Each chord has 5 spaces to work with
        let name = format!("{:^5}", cs.to_string());

        // For rendering chord notes
        let notes = cs.chord_for_key(&key).describe_notes();
        if notes.len() > required_lines {
            required_lines = notes.len();
        }
        chord_notes.push(notes);

        let style = if i == idx {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };
        Span::styled(name, style)
    }).collect();
    lines.push(Spans::from(chord_name_spans));

    for i in 0..required_lines {
        let mut cur_len = 0;
        let chord_note_spans: Vec<Span> = chord_notes.iter().enumerate().filter_map(|(j, notes)| {
            if i < notes.len() {
                let position = j * 5;
                let padding = position - cur_len;
                let padding = std::iter::repeat(' ').take(padding).collect::<String>();
                let note = format!("{}{}", padding, notes[i]);
                cur_len += note.len();
                Some(Span::raw(note))
            } else {
                None
            }
        }).collect();
        lines.push(Spans::from(chord_note_spans));
    }

    Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .style(Style::default())
        )
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(200);
    loop {
        if let Some(ref mut prog) = app.audio.progression {
            if let Some(event) = prog.event_sequence.pop_event()? {
                match event {
                    AudioEvent::Chord(i) => {
                        app.chord_idx = *i;

                        // Send MIDI data
                        // There might be some timing issues here b/c of the tick rate
                        let (chord, _) = &app.progression_in_key[*i];
                        app.midi.play_chord(chord, 1);
                    }
                }
            }
        }

        terminal.draw(|rect| {
            let size = rect.size();
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(1)
                .constraints([
                        // Main pane
                        Constraint::Ratio(if app.input_mode == InputMode::Select {
                            3
                        } else {
                            4
                        }, 4),

                        // Select pane
                        Constraint::Ratio(if app.input_mode == InputMode::Select {
                            1
                        } else {
                            0
                        }, 4),
                    ].as_ref())
                .split(size);

            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                        // To vertically center the text in the progression chunk
                        Constraint::Length(size.height/2 - 4),

                        // Progression chunk
                        Constraint::Min(2),

                        // Messages chunk
                        Constraint::Length(1),

                        // Help chunk
                        Constraint::Length(1),
                    ].as_ref())
                .split(chunks[0]);

            let param_style = Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD);
            let mut status = vec![
                Span::raw("[r]oot:"),
                Span::styled(app.key.root.to_string(), param_style),
                Span::raw(" [b]ars:"),
                Span::styled(app.bars.to_string(), param_style),
                Span::raw(" [m]ode:"),
                Span::styled(app.key.mode.to_string(), param_style),
                Span::raw(" [t]empo:"),
                Span::styled(app.tempo.to_string(), param_style),
                Span::raw(" [O]utput:"),
                Span::styled(app.output.to_string(), param_style),
            ];
            if app.output == Output::Midi {
                let port_name = if let Some(name) = &app.midi.name {
                    name.clone()
                } else {
                    "none".to_string()
                };
                status.push(
                    Span::raw(" [P]ort:"));
                status.push(
                    Span::styled(port_name, param_style));
            }
            status.push(Span::raw(if app.audio.is_paused() {
                " [p]lay"
            } else {
                " [p]ause"
            }));
            status.push(
                Span::raw(" [R]oll [q]uit"));
            let help = Paragraph::new(Spans::from(status))
                .style(Style::default())
                .alignment(Alignment::Left)
                .block(
                    Block::default()
                );

            let messages = match app.input_mode {
                InputMode::Editing | InputMode::Select => {
                    let label = match app.input_target {
                        InputTarget::Root => "Root: ",
                        InputTarget::Tempo => "Tempo: ",
                        InputTarget::Bars => "Bars: ",
                        InputTarget::MidiPort => "Port: ",
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
                InputMode::Normal => {
                    Paragraph::new(app.message)
                        .style(Style::default())
                        .alignment(Alignment::Right)
                        .block(
                            Block::default()
                        )
                },
            };

            rect.render_widget(help, main_chunks[3]);
            rect.render_widget(messages, main_chunks[2]);

            if app.input_mode == InputMode::Select {
                let mut spans = vec![];
                for (i, choice) in app.choices.iter().enumerate() {
                    spans.push(Spans::from(
                            Span::raw(format!("{}. {}", i, choice))));
                }
                let list = Paragraph::new(spans)
                    .style(Style::default())
                    .alignment(Alignment::Left)
                    .block(
                        Block::default().borders(Borders::LEFT)
                    );
                rect.render_widget(list, chunks[1]);
            }

            rect.render_widget(render_progression(&app.progression, &app.key, app.chord_idx), main_chunks[1]);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.message = "";
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('t') => {
                            app.input.clear();
                            app.input_mode = InputMode::Editing;
                            app.input_target = InputTarget::Tempo;
                        }
                        KeyCode::Char('b') => {
                            app.input.clear();
                            app.input_mode = InputMode::Editing;
                            app.input_target = InputTarget::Bars;
                        }
                        KeyCode::Char('r') => {
                            app.input.clear();
                            app.input_mode = InputMode::Editing;
                            app.input_target = InputTarget::Root;
                        }
                        KeyCode::Char('m') => {
                            app.key.mode = match app.key.mode {
                                Mode::Major => Mode::Minor,
                                Mode::Minor => Mode::Major,
                            };
                            // Mode changes require a new progression
                            app.gen_progression()?;
                        }
                        KeyCode::Char('p') => {
                            if app.audio.is_paused() {
                                app.audio.resume()?;
                            } else {
                                app.audio.pause()?;
                            }
                        }
                        KeyCode::Char('O') => {
                            match app.output {
                                Output::Audio => {
                                    app.audio.mute()?;
                                    app.output = Output::Midi;
                                    app.input_mode = InputMode::Select;
                                    app.input_target = InputTarget::MidiPort;
                                    app.choices = app.midi.available_ports().unwrap();
                                    app.input.clear();
                                }
                                Output::Midi => {
                                    app.audio.unmute()?;
                                    app.output = Output::Audio;
                                }
                            }
                        }
                        KeyCode::Char('P') => {
                            if app.output == Output::Midi {
                                app.input_mode = InputMode::Select;
                                app.input_target = InputTarget::MidiPort;
                                app.choices = app.midi.available_ports().unwrap();
                                app.input.clear();
                            }
                        }
                        KeyCode::Char('R') => {
                            app.gen_progression()?;
                        }
                        KeyCode::Char('q') => {
                            return Ok(());
                        }
                        _ => {}
                    }
                    InputMode::Select => match key.code {
                        KeyCode::Char('j') => {
                            // TODO scroll list down
                        }
                        KeyCode::Char('k') => {
                            // TODO scroll list up
                        }
                        KeyCode::Char(c) => {
                            if c.is_numeric() {
                                app.input.push(c);
                            }
                        }
                        KeyCode::Enter => {
                            let input = app.input.drain(..)
                                .collect::<String>();
                            if input.len() > 0 {
                                match app.input_target {
                                    InputTarget::MidiPort => {
                                        let idx = input.parse::<usize>()?;
                                        app.midi.connect_port(idx).unwrap();
                                    }
                                    _ => {}
                                }
                            }
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    }
                    InputMode::Editing => match key.code {
                        KeyCode::Enter => {
                            let input = app.input.drain(..)
                                .collect::<String>();
                            if input.len() > 0 {
                                match app.input_target {
                                    InputTarget::Root => {
                                        app.key.root = match input.try_into() {
                                            Ok(note) => {
                                                note
                                            }
                                            Err(_) => {
                                                app.message = "Invalid root note";
                                                app.key.root
                                            }
                                        };
                                    }
                                    InputTarget::Tempo => {
                                        app.tempo = input.parse::<usize>()?;
                                    }
                                    InputTarget::Bars => {
                                        app.bars = input.parse::<usize>()?;
                                    }
                                    _ => {}
                                }
                                app.update_progression()?;
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
            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        }
    }
}
