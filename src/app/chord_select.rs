use anyhow::Result;
use super::select::Select;
use tui::widgets::Paragraph;
use crossterm::event::KeyCode;
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

fn chord_options(root: usize) -> Vec<String> {
    let numeral = NUMERALS[root % 7].to_string();
    let maj_chords = MAJ_CHORD_TYPES.iter()
        .map(|c| format!("{}{}", numeral, c));

    let min_numeral = numeral.to_lowercase();
    let min_chords = MIN_CHORD_TYPES.iter()
        .map(|c| format!("{}{}", min_numeral, c));

    maj_chords.chain(min_chords).collect()
}

pub struct ChordSelect {
    select: Select,
}

impl Default for ChordSelect {
    fn default() -> Self {
        ChordSelect {
            select: Select {
                idx: 0,
                choices: chord_options(0),
            }
        }
    }
}

impl ChordSelect {
    pub fn render<'a>(&self, height: usize) -> Paragraph<'a> {
        self.select.render(height)
    }

    /// Process input and returns a selected ChordSpec, if any,
    /// and if the widget should be closed.
    pub fn process_input(&mut self, key: KeyCode) -> Result<(Option<ChordSpec>, bool)> {
        self.select.process_input(key);
        let idx = self.select.idx;

        let cs: ChordSpec = self.select.choices[idx].clone().try_into()?;
        match key {
            KeyCode::Char('j') | KeyCode::Char('k') | KeyCode::Char(' ') => {
                Ok((Some(cs), false))
            }
            KeyCode::Char(c) => {
                if c.is_numeric() {
                    let numeral = c.to_string().parse::<usize>()? - 1;
                    if numeral < 7 {
                        self.select.choices = chord_options(numeral);
                    }
                }
                Ok((None, false))
            }
            KeyCode::Enter => {
                Ok((Some(cs), true))
            }
            KeyCode::Esc => {
                Ok((None, true))
            }
            _ => Ok((None, false))
        }
    }
}
