use anyhow::Result;
use super::select::Select;
use super::text_input::TextInput;
use tui::widgets::Paragraph;
use crate::core::{ChordSpec, NUMERALS};
use crossterm::event::{KeyEvent, KeyCode};

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

pub struct ChordSelect<'a> {
    numeral: usize,
    select: Select,
    pub text_input: TextInput<'a>,
}

impl<'a> Default for ChordSelect<'a> {
    fn default() -> Self {
        let mut text_input = TextInput::new(
            "Chord: ",
            |_c: char| { true });

        let choices = chord_options(0);
        text_input.set_input(choices[0].to_string());
        ChordSelect {
            numeral: 0,
            text_input,
            select: Select {
                idx: 0,
                choices,
            }
        }
    }
}

impl<'a> ChordSelect<'a> {
    pub fn render<'b>(&self, height: usize) -> Paragraph<'b> {
        self.select.render(height)
    }

    /// Process input and returns a selected ChordSpec, if any,
    /// and if the widget should be closed.
    pub fn process_input(&mut self, key: KeyEvent) -> Result<(Option<ChordSpec>, bool)> {
        self.select.process_input(key)?;
        let idx = self.select.idx;

        let cs: ChordSpec = self.select.choices[idx].clone().try_into()?;
        match key.code {
            KeyCode::Char('j') | KeyCode::Char('k') | KeyCode::Char(' ') => {
                self.text_input.set_input(cs.to_string());
                Ok((Some(cs), false))
            }
            KeyCode::Char('h') => {
                if self.numeral > 0 {
                    self.numeral -= 1;
                } else {
                    self.numeral = 6;
                }
                self.text_input.set_input(cs.to_string());
                self.select.choices = chord_options(self.numeral);
                Ok((None, false))
            }
            KeyCode::Char('l') => {
                if self.numeral < 6 {
                    self.numeral += 1;
                } else {
                    self.numeral = 0;
                }
                self.text_input.set_input(cs.to_string());
                self.select.choices = chord_options(self.numeral);
                Ok((None, false))
            }
            KeyCode::Enter => {
                // TODO handle this properly
                let cs: ChordSpec = self.text_input.input.clone().try_into()?;
                Ok((Some(cs), true))
            }
            KeyCode::Esc => {
                Ok((None, true))
            }
            _ => {
                self.text_input.process_input(key)?;
                Ok((None, false))
            }
        }
    }
}
