use anyhow::Result;
use tui::{
    layout::Alignment,
    style::{Style, Color},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Borders},
};
use crossterm::event::{KeyEvent, KeyCode};

pub struct Select {
    pub idx: usize,
    pub choices: Vec<String>,
}

impl Select {
    pub fn render<'a>(&self, height: usize) -> Paragraph<'a> {
        let start = self.idx.saturating_sub(height);
        let end = self.choices.len().min(start+height);

        let mut rows = vec![];
        for (i, choice) in self.choices[start..end].iter().enumerate() {
            let choice = choice.to_string();
            let span = if i + start == self.idx {
                Span::styled(choice, Style::default().fg(Color::LightBlue))
            } else {
                Span::raw(choice)
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

    /// Process input and returns the selected index
    /// and if the widget should be closed.
    pub fn process_input(&mut self, key: KeyEvent) -> Result<(Option<usize>, bool)> {
        let n_choices = self.choices.len();
        match key.code {
            KeyCode::Char('j') => {
                if self.idx < self.choices.len() - 1 {
                    self.idx += 1;
                } else {
                    // Wrap around
                    self.idx = 0;
                }
                Ok((None, false))
            }
            KeyCode::Char('k') => {
                if self.idx > 0 {
                    self.idx -= 1;
                } else {
                    // Wrap around
                    self.idx = n_choices - 1;
                }
                Ok((None, false))
            }
            KeyCode::Enter => {
                Ok((Some(self.idx), true))
            }
            KeyCode::Esc => {
                Ok((None, true))
            }
            _ => Ok((None, false))
        }
    }
}
