use anyhow::Result;
use crate::midi::MIDI;
use std::{sync::Arc, cell::RefCell};
use crate::file::save_to_midi_file;
use crate::app::text_input::TextInput;
use crate::app::chord_select::ChordSelect;
use crate::core::{Key, Note, Mode, ChordSpec};
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
use tui::{
    widgets::Paragraph,
    text::{Span, Spans},
    style::{Style, Modifier, Color},
    layout::{Rect, Alignment, Constraint, Direction, Layout},
};

enum InputMode<'a> {
    Normal,
    Text(TextInput<'a>, TextTarget),
    Chord(ChordSelect<'a>, usize),
}

enum TextTarget {
    Root,
    Export,
}

pub struct Performance<'a> {
    midi: Arc<RefCell<MIDI>>,

    key: Key,
    mappings: [Option<ChordSpec>; 9],

    save_dir: String,
    input_mode: InputMode<'a>,

    // Last status message
    message: &'a str,
}

impl<'a> Performance<'a> {
    pub fn new(midi: Arc<RefCell<MIDI>>, save_dir: String) -> Performance<'a> {
        let key = Key {
            mode: Mode::Major,
            root: Note {
                semitones: 39
            }, // C4
        };
        Performance {
            key,
            midi,
            save_dir,
            mappings: Default::default(),
            message: "",
            input_mode: InputMode::Normal,
        }
    }

    pub fn capture_input(&self) -> bool {
        match self.input_mode {
            InputMode::Normal => false,
            _ => true
        }
    }

    pub fn render(&mut self, rect: Rect) -> Vec<(Paragraph, Rect)> {
        let mut rects = vec![];

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
            InputMode::Chord(select, _) => select.text_input.render(),
            _ => Paragraph::new(self.message)
                .alignment(Alignment::Right)
        };
        rects.push((message, chunks[1]));

        match &self.input_mode {
            InputMode::Chord(select, idx) => {
                let display_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .margin(2)
                    .constraints([
                            // Progression chunk
                            Constraint::Ratio(1, 2),

                            // Chord select chunk
                            Constraint::Ratio(1, 2),
                        ].as_ref())
                    .split(chunks[0]);

                let height = display_chunks[1].height as usize;
                rects.push((select.render(height), display_chunks[1]));

                rects.push((render_mappings(&self.key, &self.mappings, Some(*idx)), display_chunks[0]));
            }
            _ => {
                rects.push((render_mappings(&self.key, &self.mappings, None), chunks[0]));
            }
        }
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
                            }
                            TextTarget::Export => {
                                let chords = self.mappings.iter().map(|m| {
                                    match m {
                                        Some(cs) => Some(cs.chord_for_key(&self.key)),
                                        None => None
                                    }
                                }).collect();
                                let result = save_to_midi_file(
                                    120, // default tempo
                                    2,   // default ticks per beat
                                    &chords,
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
            InputMode::Chord(ref mut chord_select, idx) => {
                match chord_select.process_input(key) {
                    Ok((sel, close)) => {
                        if let Some(cs) = sel {
                            let chord = cs.chord_for_key(&self.key);
                            self.midi.borrow_mut().play_chord(&chord, 100);
                            self.mappings[*idx] = Some(cs);
                        }
                        if close {
                            self.input_mode = InputMode::Normal;
                        }

                        match key.code {
                            KeyCode::Char(c) => {
                                if c.is_numeric() {
                                    let idx = c.to_string().parse::<usize>()? - 1;
                                    if let Some(cs) = &self.mappings[idx] {
                                        let chord = cs.chord_for_key(&self.key);
                                        self.midi.borrow_mut().play_chord(&chord, 100);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(_) => {
                        self.message = "Invalid chord";
                        self.input_mode = InputMode::Normal;
                    }
                }
            }
            InputMode::Normal => {
                match key {
                    // Select slot to bind
                    KeyEvent {
                        modifiers: KeyModifiers::ALT,
                        code: KeyCode::Char(c),
                    } => {
                        if c.is_numeric() {
                            let idx = c.to_string().parse::<usize>()?;
                            if idx > 0 {
                                self.input_mode = InputMode::Chord(
                                    ChordSelect::default(), idx-1);
                            }
                        }

                    }
                    _ => {}
                }
                match key.code {
                    // Change root
                    KeyCode::Char('r') => {
                        self.input_mode = InputMode::Text(
                            TextInput::new("Root: ", |c: char| c.is_alphanumeric()),
                            TextTarget::Root);
                    }

                    // Start export to MIDI flow
                    KeyCode::Char('E') => {
                        let mut text_input = TextInput::new("Path: ", |_c: char| true);
                        text_input.set_input(self.save_dir.to_string());
                        self.input_mode = InputMode::Text(
                            text_input, TextTarget::Export);
                    }

                    // Play the chord bound to that number
                    KeyCode::Char(c) => {
                        if c.is_numeric() {
                            let idx = c.to_string().parse::<usize>()? - 1;
                            if let Some(cs) = &self.mappings[idx] {
                                let chord = cs.chord_for_key(&self.key);
                                self.midi.borrow_mut().play_chord(&chord, 100);
                            }
                        }
                    }

                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub fn controls<'b>(&self) -> Vec<Span<'b>> {
        let param_style = Style::default().fg(Color::LightBlue)
            .add_modifier(Modifier::BOLD);
        let controls = vec![
            Span::raw("[r]oot:"),
            Span::styled(self.key.root.to_string(), param_style),
            Span::raw(" [E]xport"),
        ];
        controls
    }
}

pub fn render_mappings<'a>(key: &Key, mappings: &[Option<ChordSpec>], selected: Option<usize>) -> Paragraph<'a> {
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

    let chord_id_spans: Vec<Span> = (0..mappings.len()).map(|i| {
        // Each chord has 5 spaces to work with
        let name = format!("{:^5}", (i+1).to_string());

        let style = if selected.is_some() && i == selected.unwrap() {
            Style::default().fg(Color::LightBlue)
        } else {
            Style::default()
        };
        Span::styled(name, style)
    }).collect();
    lines.push(Spans::from(chord_id_spans));

    // The spans for the chord
    let chord_name_spans: Vec<Span> = mappings.iter().map(|mcs| {
        let (name, notes) = match mcs {
            Some(cs) => {
                // For rendering chord notes
                let notes = cs.chord_for_key(key).describe_notes();
                if notes.len() > required_lines {
                    required_lines = notes.len();
                }

                (cs.to_string(), notes)
            }
            None => ("".to_string(), vec![])
        };

        chord_notes.push(notes);

        // Each chord has 5 spaces to work with
        let name = format!("{:^5}", name);
        Span::raw(name)
    }).collect();
    lines.push(Spans::from(chord_name_spans));

    for i in 0..required_lines {
        let chord_note_spans: Vec<Span> = chord_notes.iter().map(|notes| {
            let note = if !notes.is_empty() && i < notes.len() {
                notes[i].to_string()
            } else {
                "".to_string()
            };
            let note = format!("{:^5}", note);
            Span::raw(note)
        }).collect();
        lines.push(Spans::from(chord_note_spans));
    }

    Paragraph::new(lines)
        .alignment(Alignment::Center)
}
