use anyhow::Result;
use tui::{
    layout::Alignment,
    style::{Style, Color},
    text::{Span, Spans},
    widgets::{Block, Paragraph},
};
use crossterm::event::{KeyEvent, KeyCode};

pub struct TextInput<'a> {
    pub input: String,
    label: &'a str,
    valid_chars: fn(char) -> bool,
}

impl<'a> TextInput<'a> {
    pub fn new(label: &'a str, valid_chars: fn(char) -> bool) -> TextInput {
        TextInput {
            label,
            valid_chars,
            input: "".to_string(),
        }
    }

    pub fn set_input(&mut self, input: String) {
        self.input = input;
    }

    pub fn render<'b>(&self) -> Paragraph<'b> {
        let spans = Spans::from(vec![
            Span::raw(self.label.to_string()),
            Span::styled(self.input.clone(),
                Style::default().fg(Color::LightBlue))
        ]);
        Paragraph::new(spans)
            .style(Style::default())
            .alignment(Alignment::Right)
            .block(
                Block::default()
            )
    }

    pub fn process_input(&mut self, key: KeyEvent) -> Result<(Option<String>, bool)> {
        match key.code {
            KeyCode::Enter => {
                let input = self.input.drain(..)
                    .collect::<String>();
                if input.len() > 0 {
                    Ok((Some(input), true))
                } else {
                    Ok((None, true))
                }
            }
            KeyCode::Char(c) => {
                if (self.valid_chars)(c) {
                    self.input.push(c);
                }
                Ok((None, false))
            }
            KeyCode::Backspace => {
                self.input.pop();
                Ok((None, false))
            }
            KeyCode::Esc => Ok((None, true)),
            _ => Ok((None, false))
        }
    }
}
