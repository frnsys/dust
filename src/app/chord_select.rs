use anyhow::Result;
use tui::{
    layout::Alignment,
    style::{Style, Color},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Borders},
};
use crossterm::event::KeyCode;
use super::{App, InputTarget, InputMode};
use crate::core::{ChordSpec, NUMERALS};

const MAJ_CHORD_TYPES: [&str; 16] = [
    "", ":6", ":6,9", ":7", ":7,9",
    ":b7", ":b7,9", ":b9", ":9",
    "+", "+:b7", "+:9",
    "_", "^", "^:b7,9",
    "5",
];

const MIN_CHORD_TYPES: [&str; 9] = [
    "", ":#6", ":7", ":7,#9", ":#7", ":#7,#9",
    "-", "-:b7", "-:7"
];

pub struct ChordSelectState {
    idx: usize,
    chords: Vec<String>,
}

impl Default for ChordSelectState {
    fn default() -> Self {
        ChordSelectState {
            idx: 0,
            chords: chord_options(0),
        }
    }
}

fn chord_options(root: usize) -> Vec<String> {
    let numeral = NUMERALS[root % 7].to_string();
    let maj_chords = MAJ_CHORD_TYPES.iter()
        .map(|c| format!("{}{}", numeral, c));

    let min_numeral = numeral.to_lowercase();
    let min_chords = MIN_CHORD_TYPES.iter()
        .map(|c| format!("{}{}", min_numeral, c));

    maj_chords.chain(min_chords).collect()
        // .map(|cs| cs.try_into().unwrap())
        // .collect()
}

pub fn render<'a>(app: &App, size: (usize, usize)) -> Paragraph<'a> {
    let (_width, height) = size;
    let state = &app.chord_select;

    // Figure out what chord to start at
    let start = state.idx.saturating_sub(height);
    let end = state.chords.len().min(start+height);

    let mut rows = vec![];
    for (i, chord) in state.chords[start..end].iter().enumerate() {
        let chord = chord.to_string();
        let span = if i == state.idx {
            Span::styled(chord, Style::default().fg(Color::LightBlue))
        } else {
            Span::raw(chord)
        };
        let row = Spans::from(span);
        rows.push(row);
    }
    Paragraph::new(rows)
        .style(Style::default())
        .alignment(Alignment::Left)
        .block(
            Block::default().borders(Borders::LEFT)
        )
}

pub fn process_input(app: &mut App, key: KeyCode) -> Result<()> {
    let mut state = &mut app.chord_select;
    let n_chords = MAJ_CHORD_TYPES.len() + MIN_CHORD_TYPES.len();

    match key {
        KeyCode::Char('j') => {
            if state.idx < n_chords - 1 {
                state.idx += 1;
            } else {
                // Wrap around
                state.idx = 0;
            }
            let cs: ChordSpec = state.chords[state.idx].clone().try_into()?;
            let chord = cs.chord_for_key(&app.key);
            app.audio.play_chord(&chord)?;
        }
        KeyCode::Char('k') => {
            if state.idx > 0 {
                state.idx -= 1;
            } else {
                // Wrap around
                state.idx = n_chords - 1;
            }
            let cs: ChordSpec = state.chords[state.idx].clone().try_into()?;
            let chord = cs.chord_for_key(&app.key);
            app.audio.play_chord(&chord)?;
        }
        KeyCode::Char('h') => {
            // TODO select left
        }
        KeyCode::Char('l') => {
            // TODO select right
        }
        KeyCode::Char(' ') => {
            let cs: ChordSpec = state.chords[state.idx].clone().try_into()?;
            let chord = cs.chord_for_key(&app.key);
            app.audio.play_chord(&chord)?;
        }
        KeyCode::Char('q') | KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Char(c) => {
            if c.is_numeric() {
                let numeral = c.to_string().parse::<usize>()? - 1;
                if numeral < 7 {
                    state.chords = chord_options(numeral);
                }
            }
        }
        KeyCode::Enter => {
            match app.input_target {
                InputTarget::Chord(i) => {
                    let chord_spec: Result<ChordSpec, _> = state.chords[state.idx].clone().try_into();
                    match chord_spec {
                        Ok(cs) => {
                            app.progression.set_chord(i, cs);
                        }
                        Err(_) => {
                            app.message = "Invalid chord";
                        }
                    }
                }
                _ => {}
            }
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
    Ok(())
}
