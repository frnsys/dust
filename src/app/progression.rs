use anyhow::Result;
use tui::{
    layout::Alignment,
    style::{Style, Color, Modifier},
    text::{Span, Spans},
    widgets::{Block, Paragraph},
};
use crossterm::event::KeyCode;
use super::{App, InputTarget, InputMode};


pub fn render<'a>(app: &App) -> Paragraph<'a> {
    let progression = app.progression.chords();
    let selected_chord = match app.input_mode {
        InputMode::Chord | InputMode::Sequence => {
            match app.input_target {
                InputTarget::Chord(i) => Some(i),
                InputTarget::Sequence => {
                    let (seq_idx, seq_item) = app.selected();
                    if seq_item.is_some() {
                        let chord_idx = app.progression.seq_idx_to_chord_idx(seq_idx);
                        Some(chord_idx)
                    } else {
                        None
                    }
                },
                _ => None
            }
        }
        _ => None
    };

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

        let style = if selected_chord.is_some() && i == selected_chord.unwrap() {
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
        let notes = cs.chord_for_key(&app.key).describe_notes();
        if notes.len() > required_lines {
            required_lines = notes.len();
        }
        chord_notes.push(notes);

        let style = if i == app.chord_idx {
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

pub fn process_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Char('e') => {
            app.input_mode = InputMode::Text;
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
    Ok(())
}

pub fn status<'a>(app: &App) -> Vec<Span<'a>> {
    vec![
        Span::raw("[p]in [e]dit [k]:up [j]:down [q]:back"),
    ]
}
