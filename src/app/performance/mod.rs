use anyhow::Result;
use crate::midi::MIDIOutput;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use crate::file::save_to_midi_file;
use crate::app::text_input::TextInput;
use crate::app::chord_select::ChordSelect;
use crate::app::select::Select;
use crate::progression::ProgressionTemplate;
use crate::core::{Key, Mode, Duration, Chord, ChordSpec, ChordParseError, voice_lead};
use crate::strum::{StrumLibrary, Strummer, StrokeEvent, Stroke, NoteEvent, render_strummed, render_block};

/// How long one unit of `note_duration` lasts.
const DURATION_UNIT_MS: f64 = 150.0;

/// Tempo assumed for exports and strum timing in this mode.
const BPM: f64 = 120.0;
use ratatui::crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
use ratatui::{
    text::{Span, Line},
    style::{Style, Modifier, Color},
    widgets::{Block, Paragraph, Borders},
    layout::{Rect, Alignment, Constraint, Direction, Layout},
};

enum InputMode<'a> {
    Normal,
    Text(TextInput<'a>, TextTarget),
    Chord(ChordSelect<'a>, usize),
    Select(Select, SelectTarget),
}

enum SelectTarget {
    Strum,
}

enum TextTarget {
    Root,
    Duration,
    Progression,
    Export,
}

pub struct Performance<'a> {
    midi: Arc<Mutex<MIDIOutput>>,

    key: Key,
    note_duration: u64,
    mappings: [Option<ChordSpec>; 9],

    save_dir: String,
    input_mode: InputMode<'a>,

    // Last status message
    message: &'a str,

    template: ProgressionTemplate,

    strum_library: StrumLibrary,
    /// Index of the selected strum preset, if strumming is on.
    strum: Option<usize>,
    strummer: Strummer,
}

/// A single down stroke of `chord`, held for `hold_ms`.
fn strum_once(strummer: &mut Strummer, chord: &Chord, hold_ms: f64) -> Vec<NoteEvent> {
    strummer.reset();
    let stroke = StrokeEvent { beat: 0.0, stroke: Stroke::Down, accent: false };
    let beat_ms = 60_000.0 / BPM;
    let mut events = strummer.stroke(&stroke, Some(&chord.midi_notes()), 0.0, beat_ms, hold_ms / beat_ms);
    events.extend(strummer.release_all(hold_ms));
    events
}

impl<'a> Performance<'a> {
    pub fn new(midi: Arc<Mutex<MIDIOutput>>, template: ProgressionTemplate, strum_library: StrumLibrary, save_dir: String) -> Performance<'a> {
        let key = Key::default();
        Performance {
            key,
            midi,
            save_dir,
            note_duration: 5,
            mappings: Default::default(),
            message: "",
            input_mode: InputMode::Normal,
            template,
            strummer: Strummer::from_entropy(strum_library.defaults.clone()),
            strum_library,
            strum: None,
        }
    }

    fn set_strum(&mut self, idx: Option<usize>) {
        self.strum = idx.filter(|i| *i < self.strum_library.presets.len());
        if let Some(i) = self.strum {
            self.strummer = Strummer::from_entropy(self.strum_library.presets[i].params.clone());
        }
    }

    fn strum_name(&self) -> String {
        match self.strum {
            Some(i) => self.strum_library.presets[i].name.clone(),
            None => "off".to_string(),
        }
    }

    /// Play a chord: a strummed down stroke if strumming is on, else a block chord.
    fn play_chord(&mut self, chord: &Chord) {
        let mut midi = self.midi.lock().unwrap();
        if self.strum.is_some() {
            let hold = self.note_duration as f64 * DURATION_UNIT_MS;
            let events = strum_once(&mut self.strummer, chord, hold);
            midi.schedule(Instant::now(), &events);
        } else {
            midi.play_chord(chord, self.note_duration);
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
            InputMode::Select(select, _) => {
                let display_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .margin(2)
                    .constraints([
                            Constraint::Ratio(1, 2),
                            Constraint::Ratio(1, 2),
                        ].as_ref())
                    .split(chunks[0]);
                let height = display_chunks[1].height as usize;
                rects.push((select.render(height), display_chunks[1]));
                rects.push((render_mappings(&self.key, &self.mappings, None), display_chunks[0]));
            }
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
            InputMode::Select(select, target) => {
                let (selection, close) = select.process_input(key)?;
                if close {
                    if let Some(selected) = selection {
                        match target {
                            SelectTarget::Strum => {
                                // First choice is "off"
                                self.set_strum(selected.checked_sub(1));
                            }
                        }
                    }
                    self.input_mode = InputMode::Normal;
                }
            }
            InputMode::Text(text_input, target) => {
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
                            TextTarget::Duration => {
                                self.note_duration = input.parse::<u64>()?;
                            }
                            TextTarget::Progression => {
                                let mappings: Result<Vec<ChordSpec>, ChordParseError> = input.split_whitespace()
                                    .take(9).map(|cs_str| cs_str.try_into()).collect();
                                if let Ok(chord_specs) = mappings {
                                    for (i, cs) in chord_specs.into_iter().enumerate() {
                                        self.mappings[i] = Some(cs);
                                    }
                                } else {
                                    self.message = "Invalid chord";
                                }
                            }
                            TextTarget::Export => {
                                // One chord per beat
                                let steps: Vec<Option<Vec<u8>>> = self.mappings.iter()
                                    .map(|m| m.as_ref().map(|cs| cs.chord_for_key(&self.key).midi_notes()))
                                    .collect();
                                let events = match self.strum {
                                    Some(i) => {
                                        let preset = &self.strum_library.presets[i];
                                        render_strummed(&steps, 1, &preset.pattern, &preset.params, BPM, 1, rand::random())
                                    }
                                    None => render_block(&steps, 1, BPM, 60_000.0 / BPM, 100, 1),
                                };
                                let result = save_to_midi_file(BPM, &events, input);
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
            InputMode::Chord(chord_select, idx) => {
                match chord_select.process_input(key) {
                    Ok((sel, close)) => {
                        let idx = *idx;
                        if let Some(cs) = sel {
                            let chord = cs.chord_for_key(&self.key);
                            self.mappings[idx] = Some(cs);
                            self.play_chord(&chord);
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
                                        self.play_chord(&chord);
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
                        ..
                    } => {
                        if c.is_numeric() {
                            let idx = c.to_string().parse::<usize>()?;
                            if idx > 0 {
                                let select = if let Some(cs) = &self.mappings[idx-1] {
                                    ChordSelect::with_chord(cs)
                                } else {
                                    ChordSelect::default()
                                };
                                self.input_mode = InputMode::Chord(
                                    select, idx-1);
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

                    // Change duration
                    KeyCode::Char('u') => {
                        self.input_mode = InputMode::Text(
                            TextInput::new("Duration: ", |c: char| c.is_numeric()),
                            TextTarget::Duration);
                    }

                    // Change mode
                    KeyCode::Char('m') => {
                        self.key.mode = match self.key.mode {
                            Mode::Major => Mode::Minor,
                            Mode::Minor => Mode::Major,
                        };
                    }

                    // Choose a strum pattern
                    KeyCode::Char('t') => {
                        let mut choices = vec!["off".to_string()];
                        choices.extend(self.strum_library.names());
                        let mut select = Select::new(choices);
                        select.idx = self.strum.map(|i| i + 1).unwrap_or(0);
                        self.input_mode = InputMode::Select(select, SelectTarget::Strum);
                    }

                    // Enter a progression, space-delimited
                    KeyCode::Char('p') => {
                        self.input_mode = InputMode::Text(
                            TextInput::new("Progression: ", |_c: char| true),
                            TextTarget::Progression);
                    }

                    // Apply voice leading algorithm to progression
                    KeyCode::Char('v') => {
                        // Kind of messy
                        let cses = self.mappings.iter().flatten().cloned().collect();
                        let mut vl_prog = voice_lead(&cses);
                        for maybe_cs in self.mappings.iter_mut() {
                            *maybe_cs = match maybe_cs {
                                Some(_) => Some(vl_prog.remove(0)),
                                None => None
                            }
                        }
                    }

                    // Generate a new random progression
                    KeyCode::Char('R') => {
                        let progression = self.template.gen_progression(&self.key.mode, 8, &Duration::Quarter);
                        for (i, cs) in progression.sequence.into_iter().flatten().take(9).enumerate() {
                            self.mappings[i] = Some(cs);
                        }
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
                                self.play_chord(&chord);
                            }
                        }
                    }

                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub fn params<'b>(&self) -> Vec<Span<'b>> {
        let param_style = Style::default().fg(Color::LightBlue)
            .add_modifier(Modifier::BOLD);
        let params = vec![
            Span::raw("[r]oot:"),
            Span::styled(self.key.root.to_string(), param_style),
            Span::raw(" d[u]ration:"),
            Span::styled(self.note_duration.to_string(), param_style),
            Span::raw(" [m]ode:"),
            Span::styled(self.key.mode.to_string(), param_style),
            Span::raw(" s[t]rum:"),
            Span::styled(self.strum_name(), param_style),
        ];
        params
    }

    pub fn controls<'b>(&self) -> Vec<Span<'b>> {
        let controls = vec![
            Span::raw(" [p]rogression"),
            Span::raw(" [v]oice-lead"),
            Span::raw(" [E]xport"),
            Span::raw(" [R]andom"),
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
    lines.push(Line::from(chord_id_spans));

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
    lines.push(Line::from(chord_name_spans));

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
        lines.push(Line::from(chord_note_spans));
    }

    Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title("Mappings")
                .borders(Borders::TOP)
                .style(Style::default())
        )
}
