use anyhow::Result;
use tui::{
    layout::Alignment,
    style::Style,
    text::{Span, Spans},
    widgets::{Block, Paragraph, Borders},
};
use crossterm::event::KeyCode;
use super::{App, InputTarget, InputMode};

pub fn render<'a>(app: &App) -> Paragraph<'a> {
    let mut spans = vec![];
    for (i, choice) in app.choices.iter().enumerate() {
        spans.push(Spans::from(
                Span::raw(format!("{}. {}", i, choice))));
    }
    Paragraph::new(spans)
        .style(Style::default())
        .alignment(Alignment::Left)
        .block(
            Block::default().borders(Borders::LEFT)
        )
}

pub fn process_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
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
    Ok(())
}
