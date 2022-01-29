mod grid;
mod events;
mod progression;

use anyhow::Result;
use std::{sync::Arc, cell::RefCell};
use crate::midi::MIDI;
use crate::file::save_to_midi_file;
use crate::app::text_input::TextInput;
use crate::app::chord_select::ChordSelect;
use crate::core::{Key, Note, Mode, ChordSpec};
use crate::progression::{Progression, ProgressionTemplate};
use tui::{
    text::Span,
    widgets::Paragraph,
    style::{Style, Modifier, Color},
    layout::{Rect, Alignment, Constraint, Direction, Layout},
};
use crossterm::event::{KeyEvent, KeyCode};
use events::EventEmitter;

enum InputMode<'a> {
    Normal,
    Text(TextInput<'a>, TextTarget),
    Chord(ChordSelect<'a>, ChordTarget),
}

enum ChordTarget {
    Seed,
    Chord
}

pub enum TextTarget {
    Root,
    Tempo,
    Bars,
    Export,
}

pub struct Sequencer<'a> {
    midi: Arc<RefCell<MIDI>>,

    tick: usize,
    seq_pos: (usize, usize),
    clip: (usize, usize),

    save_dir: String,
    input_mode: InputMode<'a>,

    // Params
    tempo: usize,
    bars: usize,
    key: Key,

    progression: Progression,
    template: ProgressionTemplate,
    events: EventEmitter,

    // Last status message
    message: &'a str,
}

impl<'a> Sequencer<'a> {
    pub fn new(midi: Arc<RefCell<MIDI>>, template: ProgressionTemplate, save_dir: String) -> Sequencer<'a> {
        let bars = 2;
        let key = Key {
            mode: Mode::Major,
            root: Note {
                semitones: 39
            }, // C4
        };
        let progression = template.gen_progression(bars, &key.mode);

        let mut seq = Sequencer {
            midi,

            tick: 0,
            seq_pos: (0, 0),
            clip: (0, 0),

            save_dir,
            message: "",
            input_mode: InputMode::Normal,

            bars,
            key,
            tempo: 100,
            template,
            progression,

            events: EventEmitter::new().unwrap(),
        };
        seq.gen_progression().unwrap();
        seq
    }

    pub fn has_loop(&self) -> bool {
        let (a, b) = self.clip;
        let a_clip = a > 0;
        let b_clip = b < self.progression.sequence.len();
        a_clip || b_clip
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

    /// Generates and plays a new random progression.
    fn gen_progression(&mut self) -> Result<()> {
        self.progression = self.template.gen_progression(self.bars, &self.key.mode);
        self.reset_clip();
        self.restart_events()?;
        Ok(())
    }

    /// Generates and plays a new random progression,
    /// starting with a specific chord.
    fn gen_progression_from_seed(&mut self, chord: &ChordSpec) -> Result<()> {
        self.progression = self.template.gen_progression_from_seed(chord, self.bars, &self.key.mode);
        self.reset_clip();
        self.restart_events()?;
        Ok(())
    }

    /// Updates and plays the current progression with the current key and tempo.
    fn restart_events(&mut self) -> Result<()> {
        self.events.stop()?;
        self.events.start(self.tempo as f64, self.progression.time_unit)?;

        // Reset tick position
        self.tick = 0;

        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        self.events.pause()
    }

    pub fn resume(&mut self) -> Result<()> {
        self.events.resume()
    }

    pub fn capture_input(&self) -> bool {
        match self.input_mode {
            InputMode::Normal => false,
            _ => true
        }
    }

    pub fn render(&mut self, rect: Rect) -> Vec<(Paragraph, Rect)> {
        let mut rects = vec![];

        let clip_start = self.clip_start();
        let clip_len = self.clip_len();
        if let Some(_) = self.events.pop_event().unwrap() {
            if self.tick >= clip_len {
                self.tick = 0;
            }
            self.tick += 1;
            let i = self.tick + clip_start;

            if let Some(chord_spec) = &self.progression.sequence[i-1] {
                // Send MIDI data
                // There might be some timing issues here b/c of the tick rate
                let chord = chord_spec.chord_for_key(&self.key);
                self.midi.borrow_mut().play_chord(&chord, 60);
            }
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                // Main
                Constraint::Min(6),

                // Messages/input chunk
                Constraint::Length(1),
            ].as_ref())
            .split(rect);

        let message = match &self.input_mode {
            InputMode::Text(ti, _) => ti.render(),
            _ => Paragraph::new(self.message)
                .alignment(Alignment::Right)
        };
        rects.push((message, chunks[1]));

        let display_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(2)
            .constraints([
                    // Progression chunk
                    Constraint::Ratio(1, 2),

                    // Sequence chunk
                    Constraint::Ratio(1, 2),
                ].as_ref())
            .split(chunks[0]);

        rects.push((grid::render(&self), display_chunks[0]));

        let right_pane = match &self.input_mode {
            InputMode::Chord(select, _) => {
                let height = display_chunks[1].height as usize;
                select.render(height)
            },
            _ => progression::render(&self)
        };
        rects.push((right_pane, display_chunks[1]));
        rects
    }

    pub fn process_input(&mut self, key: KeyEvent) -> Result<()> {
        match &mut self.input_mode {
            InputMode::Text(ref mut text_input, target) => {
                let (input, close) = text_input.process_input(key)?;
                if close {
                    if let Some(input) = input {
                        match target {
                            TextTarget::Root => {
                                self.key.root = match input.try_into() {
                                    Ok(note) => {
                                        note
                                    }
                                    Err(_) => {
                                        self.message = "Invalid root note";
                                        self.key.root
                                    }
                                };
                                self.restart_events()?;
                            }
                            TextTarget::Tempo => {
                                self.tempo = input.parse::<usize>()?;
                                self.restart_events()?;
                            }
                            TextTarget::Bars => {
                                self.bars = input.parse::<usize>()?;
                                self.gen_progression()?;
                            }
                            TextTarget::Export => {
                                let result = save_to_midi_file(
                                    self.tempo,
                                    self.template.ticks_per_beat(),
                                    &self.progression.in_key(&self.key),
                                    input);
                                match result {
                                    Ok(_) => {
                                        self.message = "Saved file";
                                    },
                                    Err(_) => {
                                        self.message = "Failed to save";
                                    }
                                }
                            }
                        }
                    }
                    self.input_mode = InputMode::Normal;
                }
            }
            InputMode::Chord(ref mut chord_select, target) => {
                match chord_select.process_input(key) {
                    Ok((sel, close)) => {
                        if close {
                            if let Some(cs) = sel {
                                match target {
                                    ChordTarget::Seed => {
                                        self.gen_progression_from_seed(&cs)?;
                                    },
                                    ChordTarget::Chord => {
                                        let (idx, _) = self.selected();
                                        self.progression.sequence[idx] = Some(cs);
                                    }
                                }
                            }
                            self.progression.update_chords();
                            self.input_mode = InputMode::Normal;
                        } else if let Some(cs) = sel {
                            let chord = cs.chord_for_key(&self.key);
                            self.midi.borrow_mut().play_chord(&chord, 1);
                        }
                    }
                    Err(_) => {
                        self.message = "Invalid chord";
                        self.input_mode = InputMode::Normal;
                    }
                }
            }
            InputMode::Normal => {
                match key.code {
                    // Change tempo
                    KeyCode::Char('t') => {
                        self.input_mode = InputMode::Text(
                            TextInput::new("Tempo: ", |c: char| c.is_numeric()),
                            TextTarget::Tempo);
                    }

                    // Change bars
                    KeyCode::Char('b') => {
                        self.input_mode = InputMode::Text(
                            TextInput::new("Bars: ", |c: char| c.is_numeric()),
                            TextTarget::Bars);
                    }

                    // Change root
                    KeyCode::Char('r') => {
                        self.input_mode = InputMode::Text(
                            TextInput::new("Root: ", |c: char| c.is_alphanumeric()),
                            TextTarget::Root);
                    }

                    // Change mode
                    KeyCode::Char('m') => {
                        self.key.mode = match self.key.mode {
                            Mode::Major => Mode::Minor,
                            Mode::Minor => Mode::Major,
                        };
                        // Mode changes require a new progression
                        self.gen_progression()?;
                    }

                    // Pause/resume playback
                    KeyCode::Char('p') => {
                        if self.events.is_paused() {
                            self.events.resume()?;
                        } else {
                            self.events.pause()?;
                        }
                    }

                    // Generate a new random progression
                    KeyCode::Char('R') => {
                        self.gen_progression()?;
                    }

                    // Generate a new progression with
                    // a seed chord
                    KeyCode::Char('S') => {
                        self.input_mode = InputMode::Chord(
                            ChordSelect::default(), ChordTarget::Seed);
                    }

                    // Toggle metronome sound
                    KeyCode::Char('T') => {
                        self.events.toggle_tick()?;
                    }

                    // Start export to MIDI flow
                    KeyCode::Char('E') => {
                        let mut text_input = TextInput::new("Path: ", |_c: char| true);
                        text_input.set_input(self.save_dir.to_string());
                        self.input_mode = InputMode::Text(
                            text_input, TextTarget::Export);
                    }

                    // Select a progression chord by number
                    KeyCode::Char(c) => {
                        if c.is_numeric() {
                            let idx = c.to_string().parse::<usize>()? - 1;
                            if let Some(_) = self.progression.chord(idx) {
                                let seq_idx = self.progression.chord_index[idx];

                                let i = seq_idx/self.template.resolution;
                                let j = seq_idx.rem_euclid(self.template.resolution);
                                self.seq_pos = (j, i);
                            }
                        }
                    }

                    _ => {}
                }

                grid::process_input(self, key)?;
                progression::process_input(self, key)?;
            }
        }
        Ok(())
    }

    pub fn controls<'b>(&self) -> Vec<Span<'b>> {
        let param_style = Style::default().fg(Color::LightBlue)
            .add_modifier(Modifier::BOLD);
        let mut controls = vec![
            Span::raw("[r]oot:"),
            Span::styled(self.key.root.to_string(), param_style),
            Span::raw(" [b]ars:"),
            Span::styled(self.bars.to_string(), param_style),
            Span::raw(" [m]ode:"),
            Span::styled(self.key.mode.to_string(), param_style),
            Span::raw(" [t]empo:"),
            Span::styled(self.tempo.to_string(), param_style),
        ];

        controls.push(Span::raw(if self.events.is_paused() {
            " [p]lay"
        } else {
            " [p]ause"
        }));
        controls.extend(grid::controls(&self));
        controls.extend(progression::controls(&self));
        controls.push(
            Span::raw(" [T]ick [R]oll [S]eed [E]xport"));
        controls
    }
}
