use anyhow::Result;
use tui::{
    layout::Alignment,
    style::{Style, Color},
    text::{Span, Spans},
    widgets::{Block, Paragraph},
};
use crate::core::ChordSpec;
use crate::file::save_to_midi_file;
use crossterm::event::KeyCode;
use super::{App, InputTarget, InputMode};


pub fn render<'a>(app: &App) -> Paragraph<'a> {
    let label = match app.input_target {
        InputTarget::Root => "Root: ",
        InputTarget::Tempo => "Tempo: ",
        InputTarget::Bars => "Bars: ",
        InputTarget::Chord(_) => "Chord: ",
        InputTarget::Seed => "Chord: ",
        InputTarget::Export => "Path: ",
        _ => ""
    };
    let spans = Spans::from(vec![
        Span::raw(label),
        Span::styled(app.input.clone(),
            Style::default().fg(Color::LightBlue))
    ]);
    Paragraph::new(spans)
        .style(Style::default())
        .alignment(Alignment::Right)
        .block(
            Block::default()
        )
}

pub fn process_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
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
                                app.message = "Invalid chord";
                            }
                        }
                    }
                    InputTarget::Seed => {
                        let chord_spec: Result<ChordSpec, _> = input.try_into();
                        match chord_spec {
                            Ok(cs) => {
                                app.gen_progression_from_seed(&cs)?;
                            }
                            Err(_) => {
                                app.message = "Invalid chord";
                            }
                        }
                    }
                    InputTarget::Export => {
                        let result = save_to_midi_file(
                            app.tempo,
                            app.template.ticks_per_beat(),
                            &app.progression.in_key(&app.key),
                            input);
                        match result {
                            Ok(_) => {
                                app.message = "Saved file";
                            },
                            Err(_) => {
                                app.message = "Failed to save";
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
                InputTarget::Chord(_) | InputTarget::Seed | InputTarget::Export => {
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
    }
    Ok(())
}
