mod grid;
mod state;
mod progression;

use anyhow::Result;
use std::sync::{Arc, Mutex};
use crate::core::{Duration, Mode};
use crate::file::save_to_midi_file;
use crate::app::text_input::TextInput;
use crate::app::chord_select::ChordSelect;
use crate::progression::ProgressionTemplate;
use crate::midi::{MIDIOutput, MIDIClock, MIDIError, ClockEvent};
use tui::{
    text::Span,
    widgets::Paragraph,
    style::{Style, Modifier, Color},
    layout::{Rect, Alignment, Constraint, Direction, Layout},
};
use crossterm::event::{KeyEvent, KeyCode};
use state::PlaybackState;

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
    Bars,
    Duration,
    Export,
}

pub struct Sequencer<'a> {
    midi: Arc<Mutex<MIDIOutput>>,

    clock: MIDIClock,
    state: Arc<Mutex<PlaybackState>>,

    save_dir: String,
    input_mode: InputMode<'a>,

    template: ProgressionTemplate,

    grid_pos: (usize, usize),
    ticks_per_bar: usize,

    // Last status message
    message: &'a str,
}


impl<'a> Sequencer<'a> {
    pub fn new(midi: Arc<Mutex<MIDIOutput>>, template: ProgressionTemplate, save_dir: String) -> Sequencer<'a> {
        let state = PlaybackState::new(&template);
        let ticks_per_bar = state.resolution.ticks_per_bar();

        Sequencer {
            midi,
            state: Arc::new(Mutex::new(state)),
            clock: MIDIClock::default(),

            save_dir,
            message: "",
            input_mode: InputMode::Normal,

            template,
            grid_pos: (0, 0),
            ticks_per_bar,
        }
    }

    pub fn connect_port(&mut self, idx: usize) -> Result<(), MIDIError> {
        let state = self.state.clone();
        let midi = self.midi.clone();
        self.clock.connect_port(idx, move |tick| {
            let mut s = state.lock().unwrap();
            let emit_ticks = match s.resolution {
                Duration::Quarter => 24,
                Duration::Eighth => 12,
                Duration::Sixteenth => 6,
                Duration::ThirtySecond => 3,
            };
            match tick {
                ClockEvent::Tick(i) => {
                    if i % emit_ticks == 0 {
                        // Send MIDI data
                        // There might be some timing issues here b/c of the tick rate
                        if let Some((chord, duration)) = s.current_chord() {
                            midi.lock().unwrap().play_chord(&chord, duration);
                        }
                        s.tick();
                    }
                },
                ClockEvent::Stop => {
                    s.reset_tick();
                },
                _ => {}
            }
        })
    }

    pub fn selected_idx(&self) -> usize {
        let (j, i) = self.grid_pos;
        i * self.ticks_per_bar + j
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
                        let mut s = self.state.lock().unwrap();
                        match target {
                            TextTarget::Root => {
                                s.key.root = match input.try_into() {
                                    Ok(note) => {
                                        note
                                    }
                                    Err(_) => {
                                        self.message = "Invalid root note";
                                        s.key.root
                                    }
                                };
                            }
                            TextTarget::Duration => {
                                s.note_duration = input.parse::<u64>()?;
                            }
                            TextTarget::Bars => {
                                s.bars = input.parse::<usize>()?;
                                s.gen_progression(&self.template)?;
                            }
                            TextTarget::Export => {
                                let result = save_to_midi_file(
                                    120, // TODO
                                    s.progression.resolution.ticks_per_beat(),
                                    &s.progression.in_key(&s.key),
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
                            let mut s = self.state.lock().unwrap();
                            if let Some(cs) = sel {
                                match target {
                                    ChordTarget::Seed => {
                                        s.gen_progression_from_seed(&cs, &self.template)?;
                                    },
                                    ChordTarget::Chord => {
                                        let idx = self.selected_idx();
                                        s.progression.sequence[idx] = Some(cs);
                                    }
                                }
                            }
                            s.progression.update_chords();
                            self.input_mode = InputMode::Normal;
                        } else if let Some(cs) = sel {
                            let s = self.state.lock().unwrap();
                            let chord = cs.chord_for_key(&s.key);
                            self.midi.lock().unwrap().play_chord(&chord, 1);
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
                    // Change bars
                    KeyCode::Char('b') => {
                        self.message = "";
                        self.input_mode = InputMode::Text(
                            TextInput::new("Bars: ", |c: char| c.is_numeric()),
                            TextTarget::Bars);
                    }

                    // Change root
                    KeyCode::Char('r') => {
                        self.message = "";
                        self.input_mode = InputMode::Text(
                            TextInput::new("Root: ", |c: char| c.is_alphanumeric()),
                            TextTarget::Root);
                    }

                    // Change duration
                    KeyCode::Char('u') => {
                        self.message = "";
                        self.input_mode = InputMode::Text(
                            TextInput::new("Duration: ", |c: char| c.is_numeric()),
                            TextTarget::Duration);
                    }

                    // Change mode
                    KeyCode::Char('m') => {
                        let mut s = self.state.lock().unwrap();
                        s.key.mode = match s.key.mode {
                            Mode::Major => Mode::Minor,
                            Mode::Minor => Mode::Major,
                        };
                        // Mode changes require a new progression
                        s.gen_progression(&self.template)?;
                    }

                    // Apply voice leading algorithm to progression
                    KeyCode::Char('v') => {
                        let mut s = self.state.lock().unwrap();
                        s.progression = s.progression.voice_lead();
                    }

                    // Generate a new random progression
                    KeyCode::Char('R') => {
                        let mut s = self.state.lock().unwrap();
                        s.gen_progression(&self.template)?;
                    }

                    // Generate a new progression with
                    // a seed chord
                    KeyCode::Char('S') => {
                        self.message = "";
                        self.input_mode = InputMode::Chord(
                            ChordSelect::default(), ChordTarget::Seed);
                    }

                    // Start export to MIDI flow
                    KeyCode::Char('E') => {
                        self.message = "";
                        let mut text_input = TextInput::new("Path: ", |_c: char| true);
                        text_input.set_input(self.save_dir.to_string());
                        self.input_mode = InputMode::Text(
                            text_input, TextTarget::Export);
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

        let mut controls = {
            let s = self.state.lock().unwrap();
            vec![
                Span::raw("[r]oot:"),
                Span::styled(s.key.root.to_string(), param_style),
                Span::raw(" d[u]ration:"),
                Span::styled(s.note_duration.to_string(), param_style),
                Span::raw(" [b]ars:"),
                Span::styled(s.bars.to_string(), param_style),
                Span::raw(" [m]ode:"),
                Span::styled(s.key.mode.to_string(), param_style),
                Span::raw(" [v]oice-lead"),
            ]
        };

        controls.extend(grid::controls(&self));
        controls.extend(progression::controls(&self));
        controls.push(
            Span::raw(" [R]oll [S]eed [E]xport"));
        controls
    }
}
