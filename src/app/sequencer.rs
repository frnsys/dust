use anyhow::Result;
use tui::{
    layout::Alignment,
    style::{Style, Color},
    text::{Span, Spans},
    widgets::{Block, Paragraph},
};
use crossterm::event::KeyCode;
use super::{App, InputTarget, InputMode};

pub fn render<'a>(app: &App) -> Paragraph<'a> {
    let progression = &app.progression.sequence;
    let resolution = app.template.resolution;
    let bars = app.bars;
    let cur_idx = app.tick - 1;
    let selected = match app.input_mode {
        InputMode::Sequence => Some(app.selected_tick),
        _ => None
    };

    // The lines that will be rendered.
    let mut lines = vec![];

    let mut chord_idx = 0;
    for i in 0..bars {
        let mut bars: Vec<Span> = vec![];
        for j in 0..resolution {
            let idx = i*resolution + j;
            bars.push(Span::raw("|"));
            let is_selected = selected.is_some() && (j, i) == selected.unwrap();
            let tick_char = if progression[idx].is_some() {
                chord_idx += 1;
                chord_idx.to_string()
            } else if is_selected {
                "*".to_string()
            } else if idx == cur_idx {
                "*".to_string()
            } else {
                " ".to_string()
            };
            let span = if is_selected {
                Span::styled(tick_char, Style::default().fg(Color::LightBlue))
            } else if idx == cur_idx {
                Span::styled(tick_char, Style::default().fg(Color::Yellow))
            } else {
                Span::raw(tick_char)
            };
            bars.push(span);
        }
        bars.push(Span::raw("|"));
        lines.push(Spans::from(bars));
    }

    Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .style(Style::default())
        )
}

pub fn process_input(app: &mut App, key: KeyCode) -> Result<()> {
    let (j, i) = app.selected_tick;
    let selected_idx = i*app.template.resolution + j;
    let selected_tick_item = &app.progression.sequence[selected_idx];

    match key {
        KeyCode::Char('l') => {
            let (x, _) = app.selected_tick;
            app.selected_tick.0 = if x >= app.template.resolution - 1 {
                0
            } else {
                x + 1
            };
        }
        KeyCode::Char('h') => {
            let (x, _) = app.selected_tick;
            app.selected_tick.0 = if x == 0 {
                app.template.resolution - 1
            } else {
                x - 1
            };
        }
        KeyCode::Char('j') => {
            let (_, y) = app.selected_tick;
            app.selected_tick.1 = if y >= app.progression.bars - 1{
                0
            } else {
                y + 1
            };
        }
        KeyCode::Char('k') => {
            let (_, y) = app.selected_tick;
            app.selected_tick.1 = if y == 0 {
                app.progression.bars - 1
            } else {
                y - 1
            };
        }
        KeyCode::Char('d') => {
            match selected_tick_item {
                None => {},
                Some(_) => {
                    app.progression.delete_chord_at(selected_idx);
                    app.update_progression()?;
                }
            }
        }
        KeyCode::Char('a') => {
            match selected_tick_item {
                Some(_) => {},
                None => {
                    let chord_idx = app.progression.seq_idx_to_chord_idx(selected_idx);
                    let prev_chord = app.progression.prev_chord(chord_idx);
                    let cands = app.template.next(prev_chord, &app.key.mode);
                    app.progression.insert_chord_at(selected_idx, cands[0].clone());
                    app.update_progression()?;
                }
            }
        }
        KeyCode::Char('e') => {
            match selected_tick_item {
                Some(_) => {
                    app.input_mode = InputMode::Chord;
                    let chord_idx = app.progression.seq_idx_to_chord_idx(selected_idx);
                    app.input_target = InputTarget::Chord(chord_idx);
                },
                None => {
                }
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
    Ok(())
}

pub fn status<'a>(app: &App) -> Vec<Span<'a>> {
    let (j, i) = app.selected_tick;
    let selected_idx = i*app.template.resolution + j;
    let selected_tick_item = &app.progression.sequence[selected_idx];

    let span = match selected_tick_item {
        Some(_) => Span::raw("[d]elete [e]dit"),
        None => Span::raw("[a]dd"),
    };
    vec![
        span,
        Span::raw(" [q]:back"),
    ]
}
