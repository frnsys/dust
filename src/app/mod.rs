mod select;
mod sequencer;
mod progression;
mod text_input;

use anyhow::Result;
use std::fmt;
use std::time::{Duration, Instant};
use crate::midi::MIDI;
use crate::core::{Note, Key, Mode, ChordSpec};
use crate::audio::{Audio, Event as AudioEvent};
use crate::progression::{Progression, ProgressionTemplate};
use tui::{
    Terminal,
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Style, Modifier, Color},
    text::{Span, Spans},
    widgets::{Block, Paragraph},
};
use crossterm::event::{self, Event, KeyCode};


const TICK_RATE: Duration = Duration::from_millis(200);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum InputMode {
    Normal,
    Text,
    Select,
    Chord,
    Sequence,
}

enum InputTarget {
    Root,
    Tempo,
    Bars,
    MidiPort,
    Seed,
    Chord(usize),
    Sequence,
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
    selected_tick: (usize, usize),
    choices: Vec<String>,

    /// Last status message
    message: &'a str,

    /// Params
    tempo: usize,
    bars: usize,
    key: Key,

    // Current chord in the progression
    chord_idx: usize,
    progression: Progression,
    template: ProgressionTemplate,
    tick: usize,

    // Outputs
    output: Output,
    audio: Audio,
    midi: MIDI,
}

impl<'a> App<'a> {
    pub fn new(template: ProgressionTemplate) -> App<'a> {
        let bars = 4;
        let key = Key {
            mode: Mode::Major,
            root: Note {
                semitones: 27
            }, // C3
        };
        let progression = template.gen_progression(bars, &key.mode);

        let mut app = App {
            tick: 0,
            chord_idx: 0,
            selected_tick: (0, 0),
            output: Output::Audio,
            audio: Audio::new().unwrap(),
            midi: MIDI::new(),

            input: String::new(),
            input_mode: InputMode::Normal,
            input_target: InputTarget::Tempo,
            choices: vec![],
            message: "",

            bars,
            key,
            tempo: 100,
            template,
            progression,
        };
        app.gen_progression().unwrap();
        app
    }

    /// Updates and plays the current progression with the current key and tempo.
    fn update_progression(&mut self) -> Result<()> {
        self.audio.stop_progression()?;
        self.audio.play_progression(self.tempo as f64, self.progression.time_unit, &self.progression.in_key(&self.key))?;
        self.tick = 0;
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
        self.progression = self.template.gen_progression(self.bars, &self.key.mode);
        self.update_progression()?;
        Ok(())
    }

    fn gen_progression_from_seed(&mut self, chord: &ChordSpec) -> Result<()> {
        self.progression = self.template.gen_progression_from_seed(chord, self.bars, &self.key.mode);
        self.update_progression()?;
        Ok(())
    }

    pub fn selected(&self) -> (usize, &Option<ChordSpec>) {
        let (j, i) = self.selected_tick;
        let idx = i*self.template.resolution + j;
        (idx, &self.progression.sequence[idx])
    }
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    let mut last_tick = Instant::now();
    loop {
        if let Some(ref mut prog) = app.audio.progression {
            if let Some(_) = prog.metronome.pop_event()? {
                if app.tick >= app.progression.sequence.len() {
                    app.tick = 0;
                }
                app.tick += 1;
            }
            if let Some(event) = prog.event_sequence.pop_event()? {
                match event {
                    AudioEvent::Chord(i) => {
                        app.chord_idx = app.progression.seq_idx_to_chord_idx(*i);

                        // Send MIDI data
                        // There might be some timing issues here b/c of the tick rate
                        if let Some(chord_spec) = &app.progression.sequence[*i] {
                            let chord = chord_spec.chord_for_key(&app.key);
                            app.midi.play_chord(&chord, 1);
                        }
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
                    // Progression chunk
                    Constraint::Min(10),

                    // Sequence chunk
                    Constraint::Min(10),

                    // Messages chunk
                    Constraint::Length(1),

                    // Help chunk
                    Constraint::Length(1),
                ].as_ref())
                .split(chunks[0]);

            let status = match app.input_mode {
                InputMode::Chord => progression::status(&app),
                InputMode::Sequence => sequencer::status(&app),
                _ => status(&app),
            };
            let help = Paragraph::new(Spans::from(status))
                .style(Style::default())
                .alignment(Alignment::Left)
                .block(
                    Block::default()
                );

            let messages = match app.input_mode {
                InputMode::Text => text_input::render(&app),
                _ => {
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
                rect.render_widget(select::render(&app), chunks[1]);
            }

            rect.render_widget(progression::render(&app), main_chunks[0]);
            rect.render_widget(sequencer::render(&app), main_chunks[1]);
        })?;

        let timeout = TICK_RATE
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.message = "";
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => {
                            // Quit
                            return Ok(());
                        },
                        _ => process_input(&mut app, key.code)?
                    },
                    InputMode::Text => text_input::process_input(&mut app, key.code)?,
                    InputMode::Select => select::process_input(&mut app, key.code)?,
                    InputMode::Chord => progression::process_input(&mut app, key.code)?,
                    InputMode::Sequence => sequencer::process_input(&mut app, key.code)?
                }
            }
            if last_tick.elapsed() >= TICK_RATE {
                last_tick = Instant::now();
            }
        }
    }
}


pub fn status<'a>(app: &App) -> Vec<Span<'a>> {
    let param_style = Style::default().fg(Color::LightBlue)
        .add_modifier(Modifier::BOLD);
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
        Span::raw(" [M]etrn [s]equence [R]oll [S]eed [q]uit"));
    status
}

pub fn process_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Char('t') => {
            app.input.clear();
            app.input_mode = InputMode::Text;
            app.input_target = InputTarget::Tempo;
        }
        KeyCode::Char('b') => {
            app.input.clear();
            app.input_mode = InputMode::Text;
            app.input_target = InputTarget::Bars;
        }
        KeyCode::Char('r') => {
            app.input.clear();
            app.input_mode = InputMode::Text;
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
        KeyCode::Char('S') => {
            app.input_mode = InputMode::Text;
            app.input_target = InputTarget::Seed;
            app.input.clear();
        }
        KeyCode::Char('s') => {
            app.input_mode = InputMode::Sequence;
            app.input_target = InputTarget::Sequence;
        }
        KeyCode::Char('M') => {
            app.audio.toggle_tick()?;
        }
        KeyCode::Char(c) => {
            if c.is_numeric() {
                let idx = c.to_string().parse::<usize>()? - 1;
                if let Some(_) = app.progression.chord(idx) {
                    app.input_mode = InputMode::Chord;
                    app.input_target = InputTarget::Chord(idx);
                }
            }
        }
        _ => {}
    }
    Ok(())
}
