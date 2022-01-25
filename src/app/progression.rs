use anyhow::Result;
use tui::{
    layout::Alignment,
    style::{Style, Color, Modifier},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Borders},
};
use crossterm::event::KeyCode;
use super::{App, InputTarget, InputMode};


pub fn render<'a>(app: &App) -> Paragraph<'a> {
    let progression = app.progression.chords();
    let (seq_idx, seq_item) = app.selected();
    let selected_chord = if seq_item.is_some() {
        let chord_idx = app.progression.seq_idx_to_chord_idx(seq_idx);
        Some(chord_idx)
    } else {
        None
    };
    let cur_idx = app.clip_start() + app.tick - 1;

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

        let chord_idx = app.progression.chord_index[i];
        let style = if chord_idx == cur_idx {
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
                .title("Progression")
                .borders(Borders::TOP)
                .style(Style::default())
        )
}

pub fn process_input(app: &mut App, key: KeyCode) -> Result<()> {
    let (seq_idx, seq_item) = app.selected();
    let selected_chord = if seq_item.is_some() {
        let chord_idx = app.progression.seq_idx_to_chord_idx(seq_idx);
        Some(chord_idx)
    } else {
        None
    };

    match key {
        KeyCode::Char('e') => {
            if let Some(chord_idx) = selected_chord {
                app.input_mode = InputMode::Text;
                app.input_target = InputTarget::Chord(chord_idx);
                app.input = app.progression.chord(chord_idx).unwrap().to_string();
            }
        }
        KeyCode::Char('U') => {
            // Cycle up a chord
            if let Some(chord_idx) = selected_chord {
                let prev_chord = app.progression.prev_chord(chord_idx);
                let cands = app.template.next(prev_chord, &app.key.mode);
                let current = app.progression.chord(chord_idx).unwrap();
                let idx = if let Some(idx) = cands.iter().position(|cs| cs == current) {
                    if idx == cands.len() - 1 {
                        0
                    } else {
                        idx + 1
                    }
                } else {
                    0
                };
                app.progression.set_chord(chord_idx, cands[idx].clone());
                app.update_progression()?;
            }
        }
        KeyCode::Char('D') => {
            // Cycle down a chord
            if let Some(chord_idx) = selected_chord {
                let prev_chord = app.progression.prev_chord(chord_idx);
                let cands = app.template.next(prev_chord, &app.key.mode);
                let current = app.progression.chord(chord_idx).unwrap();
                let idx = if let Some(idx) = cands.iter().position(|cs| cs == current) {
                    if idx == 0 {
                        cands.len() - 1
                    } else {
                        idx - 1
                    }
                } else {
                    0
                };
                app.progression.set_chord(chord_idx, cands[idx].clone());
                app.update_progression()?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn controls<'a>(_app: &App) -> Vec<Span<'a>> {
    vec![
        Span::raw(" [e]dit [U]p [D]own"),
    ]
}
