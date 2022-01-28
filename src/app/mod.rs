mod select;
mod sequencer;
mod progression;
mod text_input;
mod chord_select;

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
use chord_select::ChordSelectState;
use crossterm::event::{self, Event, KeyCode};


const TICK_RATE: Duration = Duration::from_millis(200);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum InputMode {
    Normal,
    Text,
    Select,
}

enum InputTarget {
    Root,
    Tempo,
    Bars,
    MidiPort,
    Seed,
    Chord(usize),
    Export,
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
    seq_pos: (usize, usize),
    choices: Vec<String>,
    save_dir: String,
    chord_select: ChordSelectState,

    /// Last status message
    message: &'a str,

    /// Params
    tempo: usize,
    bars: usize,
    key: Key,

    // Current chord in the progression
    progression: Progression,
    template: ProgressionTemplate,
    tick: usize,
    clip: (usize, usize),

    // Outputs
    output: Output,
    audio: Audio,
    midi: MIDI,
}

impl<'a> App<'a> {
    pub fn new(template: ProgressionTemplate, save_dir: String) -> App<'a> {
        let bars = 2;
        let key = Key {
            mode: Mode::Major,
            root: Note {
                semitones: 27
            }, // C3
        };
        let progression = template.gen_progression(bars, &key.mode);

        let mut app = App {
            tick: 0,
            seq_pos: (0, 0),
            clip: (0, 0),
            output: Output::Audio,
            audio: Audio::new().unwrap(),
            midi: MIDI::new(),

            input: String::new(),
            input_mode: InputMode::Normal,
            input_target: InputTarget::Tempo,
            chord_select: ChordSelectState::default(),
            choices: vec![],
            message: "",
            save_dir,

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

        let prog = self.progression.in_key(&self.key);
        self.audio.play_progression(self.tempo as f64, self.progression.time_unit, &prog[self.clip.0..self.clip.1])?;

        // Reset tick position
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
        self.reset_clip();
        self.update_progression()?;
        Ok(())
    }

    fn gen_progression_from_seed(&mut self, chord: &ChordSpec) -> Result<()> {
        self.progression = self.template.gen_progression_from_seed(chord, self.bars, &self.key.mode);
        self.reset_clip();
        self.update_progression()?;
        Ok(())
    }

    pub fn selected(&self) -> (usize, &Option<ChordSpec>) {
        let (j, i) = self.seq_pos;
        let idx = i*self.template.resolution + j;
        (idx, &self.progression.sequence[idx])
    }

    pub fn clip_len(&self) -> usize {
        self.clip.1 - self.clip.0
    }

    pub fn clip_start(&self) -> usize {
        self.clip.0
    }

    pub fn reset_clip(&mut self) {
        self.clip = (0, self.progression.sequence.len());
    }
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    let mut last_tick = Instant::now();
    loop {
        let clip_start = app.clip_start();
        let clip_len = app.clip_len();
        if let Some(ref mut prog) = app.audio.progression {
            if let Some(_) = prog.metronome.pop_event()? {
                if app.tick >= clip_len {
                    app.tick = 0;
                }
                app.tick += 1;
            }
            if let Some(event) = prog.event_sequence.pop_event()? {
                match event {
                    AudioEvent::Chord(i) => {
                        let i = i + clip_start;

                        // Send MIDI data
                        // There might be some timing issues here b/c of the tick rate
                        if let Some(chord_spec) = &app.progression.sequence[i] {
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
                    // Config chunk
                    Constraint::Length(1),

                    // To vertically center the next chunk
                    Constraint::Length(size.height/2 - 5),

                    // Progression/Sequence chunk
                    Constraint::Min(6),

                    // Messages chunk
                    Constraint::Length(1),

                    // Help chunk
                    Constraint::Length(1),
                ].as_ref())
                .split(chunks[0]);

            let display_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(2)
                .constraints([
                        // Progression chunk
                        Constraint::Ratio(1, 2),

                        // Sequence chunk
                        Constraint::Ratio(1, 2),
                    ].as_ref())
                .split(main_chunks[2]);

            let config = Paragraph::new(Spans::from(config_controls(&app)))
                .style(Style::default())
                .alignment(Alignment::Left)
                .block(
                    Block::default()
                );

            let mut controls = vec![];
            controls.append(&mut progression::controls(&app));
            controls.append(&mut sequencer::controls(&app));
            controls.append(&mut main_controls(&app));
            let help = Paragraph::new(Spans::from(controls))
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

            rect.render_widget(config, main_chunks[0]);
            rect.render_widget(help, main_chunks[4]);
            rect.render_widget(messages, main_chunks[3]);

            if app.input_mode == InputMode::Select {
                match app.input_target {
                    InputTarget::Chord(_) => {
                        let size = (chunks[1].width as usize, chunks[1].height as usize);
                        rect.render_widget(chord_select::render(&app, size), chunks[1]);
                    }
                    _ => {
                        rect.render_widget(select::render(&app), chunks[1]);
                    }
                }
            }

            rect.render_widget(progression::render(&app), display_chunks[0]);
            rect.render_widget(sequencer::render(&app), display_chunks[1]);
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
                        _ => {
                            process_input(&mut app, key.code)?;
                            sequencer::process_input(&mut app, key.code)?;
                            progression::process_input(&mut app, key.code)?;
                        }
                    },
                    InputMode::Text => text_input::process_input(&mut app, key.code)?,
                    InputMode::Select => {
                        match app.input_target {
                            InputTarget::Chord(_) => {
                                chord_select::process_input(&mut app, key.code)?;
                            }
                            _ => {
                                select::process_input(&mut app, key.code)?;
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

pub fn process_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        // Change tempo
        KeyCode::Char('t') => {
            app.input.clear();
            app.input_mode = InputMode::Text;
            app.input_target = InputTarget::Tempo;
        }

        // Change bars
        KeyCode::Char('b') => {
            app.input.clear();
            app.input_mode = InputMode::Text;
            app.input_target = InputTarget::Bars;
        }

        // Change root
        KeyCode::Char('r') => {
            app.input.clear();
            app.input_mode = InputMode::Text;
            app.input_target = InputTarget::Root;
        }

        // Change mode
        KeyCode::Char('m') => {
            app.key.mode = match app.key.mode {
                Mode::Major => Mode::Minor,
                Mode::Minor => Mode::Major,
            };
            // Mode changes require a new progression
            app.gen_progression()?;
        }

        // Pause/resume playback
        KeyCode::Char('p') => {
            if app.audio.is_paused() {
                app.audio.resume()?;
            } else {
                app.audio.pause()?;
            }
        }

        // Change output
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

        // Change the MIDI output port
        KeyCode::Char('P') => {
            if app.output == Output::Midi {
                app.input_mode = InputMode::Select;
                app.input_target = InputTarget::MidiPort;
                app.choices = app.midi.available_ports().unwrap();
                app.input.clear();
            }
        }

        // Generate a new random progression
        KeyCode::Char('R') => {
            app.gen_progression()?;
        }

        // Generate a new progression with
        // a seed chord
        KeyCode::Char('S') => {
            app.input_mode = InputMode::Text;
            app.input_target = InputTarget::Seed;
            app.input.clear();
        }

        // Toggle metronome sound
        KeyCode::Char('M') => {
            app.audio.toggle_tick()?;
        }

        // Start export to MIDI flow
        KeyCode::Char('E') => {
            app.input_mode = InputMode::Text;
            app.input_target = InputTarget::Export;
            app.input = app.save_dir.to_string();
        }

        // Chord select mode
        KeyCode::Char('s') => {
            match app.input_target {
                InputTarget::Chord(_) => (),
                _ => {
                    app.input_target = InputTarget::Chord(0);
                }
            }
            app.input_mode = InputMode::Select;
        }

        // Select a chord by number
        KeyCode::Char(c) => {
            if c.is_numeric() {
                let idx = c.to_string().parse::<usize>()? - 1;
                if let Some(_) = app.progression.chord(idx) {
                    let seq_idx = app.progression.chord_index[idx];

                    let i = seq_idx/app.template.resolution;
                    let j = seq_idx.rem_euclid(app.template.resolution);
                    app.seq_pos = (j, i);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn config_controls<'a>(app: &App) -> Vec<Span<'a>> {
    let param_style = Style::default().fg(Color::LightBlue)
        .add_modifier(Modifier::BOLD);
    let mut controls = vec![
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
        controls.push(
            Span::raw(" [P]ort:"));
        controls.push(
            Span::styled(port_name, param_style));
    }
    controls
}

pub fn main_controls<'a>(app: &App) -> Vec<Span<'a>> {
    let mut controls = vec![];
    controls.push(Span::raw(if app.audio.is_paused() {
        " [p]lay"
    } else {
        " [p]ause"
    }));
    controls.push(
        Span::raw(" [M]etrn [R]oll [S]eed [E]xport [q]uit"));
    controls
}
