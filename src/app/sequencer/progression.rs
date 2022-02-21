use anyhow::Result;
use tui::{
    layout::Alignment,
    style::{Style, Color, Modifier},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Borders},
};
use super::Sequencer;
use crossterm::event::{KeyEvent, KeyCode};

pub fn render<'a>(seq: &Sequencer) -> Paragraph<'a> {
    let sel_idx = seq.selected_idx();
    let state = seq.state.lock().unwrap();
    let progression = state.progression.chords();
    let sel_item = &state.progression.sequence[sel_idx];
    let selected_chord = if sel_item.is_some() {
        let chord_idx = state.progression.seq_idx_to_chord_idx(sel_idx);
        Some(chord_idx)
    } else {
        None
    };
    let cur_idx = state.clip_start() + state.tick;

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
        let notes = cs.chord_for_key(&state.key).describe_notes();
        if notes.len() > required_lines {
            required_lines = notes.len();
        }
        chord_notes.push(notes);

        let chord_idx = state.progression.chord_index[i];
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

pub fn process_input(seq: &mut Sequencer, key: KeyEvent) -> Result<()> {
    let sel_idx = seq.selected_idx();
    let mut state = seq.state.lock().unwrap();
    let sel_item = &state.progression.sequence[sel_idx];
    let selected_chord = if sel_item.is_some() {
        let chord_idx = state.progression.seq_idx_to_chord_idx(sel_idx);
        Some(chord_idx)
    } else {
        None
    };

    match key.code {
        KeyCode::Char('U') => {
            // Cycle up a chord
            if let Some(chord_idx) = selected_chord {
                let prev_chord = state.progression.prev_chord(chord_idx);
                let cands = seq.template.next(prev_chord, &state.key.mode);
                let current = state.progression.chord(chord_idx).unwrap();
                let idx = if let Some(idx) = cands.iter().position(|cs| cs == current) {
                    if idx == cands.len() - 1 {
                        0
                    } else {
                        idx + 1
                    }
                } else {
                    0
                };
                state.progression.set_chord(chord_idx, cands[idx].clone());
            }
        }
        KeyCode::Char('D') => {
            // Cycle down a chord
            if let Some(chord_idx) = selected_chord {
                let prev_chord = state.progression.prev_chord(chord_idx);
                let cands = seq.template.next(prev_chord, &state.key.mode);
                let current = state.progression.chord(chord_idx).unwrap();
                let idx = if let Some(idx) = cands.iter().position(|cs| cs == current) {
                    if idx == 0 {
                        cands.len() - 1
                    } else {
                        idx - 1
                    }
                } else {
                    0
                };
                state.progression.set_chord(chord_idx, cands[idx].clone());
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn controls<'a>(seq: &Sequencer) -> Vec<Span<'a>> {
    let sel_idx = seq.selected_idx();
    let state = seq.state.lock().unwrap();
    let sel_item = &state.progression.sequence[sel_idx];
    if sel_item.is_some() {
        vec![
            Span::raw(" [U]p [D]own"),
        ]
    } else {
        vec![]
    }
}
