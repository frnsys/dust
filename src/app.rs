// TODO clean this up!

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
use crate::midi::MIDI;
use crate::audio::{Audio, Event as AudioEvent};
use crate::progression::{Progression, ProgressionTemplate};
use crate::core::{Note, Key, Mode, ChordSpec};

const TICK_RATE: Duration = Duration::from_millis(200);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum InputMode {
    Normal,
    Editing,
    Select,
    Chord,
    Sequence,
}

enum InputTarget {
    Root,
    Tempo,
    Bars,
    MidiPort,
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
}

fn render_progression<'a>(progression: &Vec<&ChordSpec>, key: &Key, idx: usize, selected: Option<usize>) -> Paragraph<'a> {
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

    let chord_id_spans: Vec<Span> = (0..progression.len()).map(|i| {
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
    let chord_name_spans: Vec<Span> = progression.iter().enumerate().map(|(i, cs)| {
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
                let position = j * 5; // Each chord has 5 spaces to work with
                let padding = position - cur_len;
                let padding = std::iter::repeat(' ').take(padding).collect::<String>();
                let note = format!("{}{:^5}", padding, notes[i]);
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


fn render_timing<'a>(progression: &Vec<Option<ChordSpec>>, resolution: usize, bars: usize, cur_idx: usize, selected: Option<(usize, usize)>) -> Paragraph<'a> {
    // The lines that will be rendered.
    let mut lines = vec![];

    let beat_ids: Vec<Span> = (0..resolution).map(|i| {
        let name = format!(" {}", (i+1).to_string());
        Span::raw(name)
    }).collect();
    lines.push(Spans::from(beat_ids));

    let mut chord_idx = 0;
    for i in 0..bars {
        let mut bars: Vec<Span> = vec![];
        for j in 0..resolution {
            let idx = i*resolution + j;
            bars.push(Span::raw("|"));
            let is_selected = selected.is_some() && (j, i) == selected.unwrap();
            let tick_char = if progression[idx].is_some() {
                chord_idx += 1;
                chord_idx.to_string()
            } else if is_selected {
                "*".to_string()
            } else if idx == cur_idx {
                "*".to_string()
            } else {
                " ".to_string()
            };
            let span = if is_selected {
                Span::styled(tick_char, Style::default().fg(Color::LightBlue))
            } else if idx == cur_idx {
                Span::styled(tick_char, Style::default().fg(Color::Yellow))
            } else {
                Span::raw(tick_char)
            };
            bars.push(span);
        }
        bars.push(Span::raw("|"));
        lines.push(Spans::from(bars));
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

        let (j, i) = app.selected_tick;
        let selected_idx = i*app.template.resolution + j;
        let selected_tick_item = &app.progression.sequence[selected_idx];

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
                        // Constraint::Length(size.height/2 - 4),
                        Constraint::Length(4), // TODO

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

            let param_style = Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD);
            let status = if app.input_mode == InputMode::Chord {
                vec![
                    Span::raw("[p]in [e]dit [k]:up [j]:down [q]:back"),
                ]
            } else if app.input_mode == InputMode::Sequence {
                let span = match selected_tick_item {
                    Some(_) => Span::raw("[d]elete [e]dit"),
                    None => Span::raw("[a]dd"),
                };
                vec![
                    span,
                    Span::raw(" [q]:back"),
                ]
            } else {
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
                    Span::raw(" [s]equence [R]oll [q]uit"));
                status
            };
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
                        InputTarget::Chord(_) => "Chord: ",
                        _ => ""
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
                _ => {
                    Paragraph::new(app.message)
                        .style(Style::default())
                        .alignment(Alignment::Right)
                        .block(
                            Block::default()
                        )
                },
            };

            rect.render_widget(help, main_chunks[4]);
            rect.render_widget(messages, main_chunks[3]);

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

            let selected_chord = if app.input_mode == InputMode::Chord {
                match app.input_target {
                    InputTarget::Chord(i) => Some(i),
                    _ => None
                }
            } else {
                None
            };
            rect.render_widget(render_progression(&app.progression.chords(), &app.key, app.chord_idx, selected_chord), main_chunks[1]);

            let selected_tick = match app.input_mode {
                InputMode::Sequence => Some(app.selected_tick),
                _ => None
            };
            rect.render_widget(render_timing(&app.progression.sequence, app.template.resolution, app.bars, app.tick-1, selected_tick), main_chunks[2]);
        })?;

        let timeout = TICK_RATE
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
                        KeyCode::Char('s') => {
                            app.input_mode = InputMode::Sequence;
                            app.input_target = InputTarget::Sequence;
                        }
                        KeyCode::Char('q') => {
                            return Ok(());
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
                                        app.gen_progression()?;
                                    }
                                    InputTarget::Chord(i) => {
                                        let chord_spec: Result<ChordSpec, _> = input.try_into();
                                        match chord_spec {
                                            Ok(cs) => {
                                                app.progression.set_chord(i, cs);
                                            }
                                            Err(_) => {
                                                app.message = "Invalid chord"
                                            }
                                        }
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
                                }
                                InputTarget::Chord(_) => {
                                    app.input.push(c);
                                }
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
                    InputMode::Chord => match key.code {
                        KeyCode::Char('e') => {
                            app.input_mode = InputMode::Editing;
                        }
                        KeyCode::Char('k') => {
                            // Cycle up a chord
                            match app.input_target {
                                InputTarget::Chord(i) => {
                                    let prev_chord = app.progression.prev_chord(i);
                                    let cands = app.template.next(prev_chord, &app.key.mode);
                                    let current = app.progression.chord(i).unwrap();
                                    let idx = if let Some(idx) = cands.iter().position(|cs| cs == current) {
                                        if idx == cands.len() - 1 {
                                            0
                                        } else {
                                            idx + 1
                                        }
                                    } else {
                                        0
                                    };
                                    app.progression.set_chord(i, cands[idx].clone());
                                    app.update_progression()?;
                                }
                                _ => {}
                            }
                        }
                        KeyCode::Char('j') => {
                            // Cycle down a chord
                            match app.input_target {
                                InputTarget::Chord(i) => {
                                    let prev_chord = app.progression.prev_chord(i);
                                    let cands = app.template.next(prev_chord, &app.key.mode);
                                    let current = app.progression.chord(i).unwrap();
                                    let idx = if let Some(idx) = cands.iter().position(|cs| cs == current) {
                                        if idx == 0 {
                                            cands.len() - 1
                                        } else {
                                            idx - 1
                                        }
                                    } else {
                                        0
                                    };
                                    app.progression.set_chord(i, cands[idx].clone());
                                    app.update_progression()?;
                                }
                                _ => {}
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('q') => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char(c) => {
                            if c.is_numeric() {
                                // 1-indexed to 0-indexed
                                let idx = c.to_string().parse::<usize>()? - 1;
                                if let Some(_) = app.progression.chord(idx) {
                                    app.input_mode = InputMode::Chord;
                                    app.input_target = InputTarget::Chord(idx);
                                }
                            }
                        }
                        _ => {}
                    }
                    InputMode::Sequence => match key.code {
                        KeyCode::Char('l') => {
                            let (x, _) = app.selected_tick;
                            app.selected_tick.0 = if x >= app.template.resolution - 1 {
                                0
                            } else {
                                x + 1
                            };
                        }
                        KeyCode::Char('h') => {
                            let (x, _) = app.selected_tick;
                            app.selected_tick.0 = if x == 0 {
                                app.template.resolution - 1
                            } else {
                                x - 1
                            };
                        }
                        KeyCode::Char('j') => {
                            let (_, y) = app.selected_tick;
                            app.selected_tick.1 = if y >= app.progression.bars - 1{
                                0
                            } else {
                                y + 1
                            };
                        }
                        KeyCode::Char('k') => {
                            let (_, y) = app.selected_tick;
                            app.selected_tick.1 = if y == 0 {
                                app.progression.bars - 1
                            } else {
                                y - 1
                            };
                        }
                        KeyCode::Char('d') => {
                            match selected_tick_item {
                                None => {},
                                Some(_) => {
                                    app.progression.delete_chord_at(selected_idx);
                                    app.update_progression()?;
                                }
                            }
                        }
                        KeyCode::Char('a') => {
                            match selected_tick_item {
                                Some(_) => {},
                                None => {
                                    let chord_idx = app.progression.seq_idx_to_chord_idx(selected_idx);
                                    let prev_chord = app.progression.prev_chord(chord_idx);
                                    let cands = app.template.next(prev_chord, &app.key.mode);
                                    app.progression.insert_chord_at(selected_idx, cands[0].clone());
                                    app.update_progression()?;
                                }
                            }
                        }
                        KeyCode::Char('e') => {
                            match selected_tick_item {
                                Some(_) => {
                                    app.input_mode = InputMode::Chord;
                                    let chord_idx = app.progression.seq_idx_to_chord_idx(selected_idx);
                                    app.input_target = InputTarget::Chord(chord_idx);
                                },
                                None => {
                                }
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('q') => {
                            app.input_mode = InputMode::Normal;
                        }
                        // TODO
                        _ => {}
                    }
                }
            }
            if last_tick.elapsed() >= TICK_RATE {
                last_tick = Instant::now();
            }
        }
    }
}
