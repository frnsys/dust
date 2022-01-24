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
    let cur_idx = app.clip_start() + app.tick - 1;
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
            let mut style = Style::default();
            if is_selected {
                style = style.fg(Color::LightBlue);
            } else if idx == cur_idx {
                style = style.fg(Color::Yellow);
            };

            // Highlight loop
            let (a, b) = app.clip;
            let a_clip = a > 0;
            let b_clip = b < app.progression.sequence.len();
            let b = b - 1;
            if a_clip && a == idx {
                style = style.bg(Color::DarkGray);
            }
            if b_clip && b == idx {
                style = style.bg(Color::DarkGray);
            }
            if a_clip && b_clip {
                if a <= idx && idx <= b {
                    style = style.bg(Color::DarkGray);
                }
            }

            let span = Span::styled(tick_char, style);
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
    let (sel_idx, sel_item) = app.selected();

    match key {
        KeyCode::Char('A') => {
            if app.clip.0 != sel_idx {
                app.clip.0 = sel_idx;
                app.update_progression()?;
            }
        }
        KeyCode::Char('B') => {
            let idx = sel_idx + 1;
            if app.clip.1 != idx {
                app.clip.1 = idx;
                app.update_progression()?;
            }
        }
        KeyCode::Char('C') => {
            app.reset_clip();
            app.update_progression()?;
        }
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
            app.selected_tick.1 = if y >= app.progression.bars - 1 {
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
            match sel_item {
                None => {},
                Some(_) => {
                    app.progression.delete_chord_at(sel_idx);
                    app.update_progression()?;
                }
            }
        }
        KeyCode::Char('a') => {
            match sel_item {
                Some(_) => {},
                None => {
                    let chord_idx = app.progression.seq_idx_to_chord_idx(sel_idx);
                    let prev_chord = app.progression.prev_chord(chord_idx);
                    let cands = app.template.next(prev_chord, &app.key.mode);
                    app.progression.insert_chord_at(sel_idx, cands[0].clone());
                    app.update_progression()?;
                }
            }
        }
        KeyCode::Char('e') => {
            match sel_item {
                Some(_) => {
                    app.input_mode = InputMode::Chord;
                    let chord_idx = app.progression.seq_idx_to_chord_idx(sel_idx);
                    app.input_target = InputTarget::Chord(chord_idx);
                },
                None => {
                }
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.input_mode = InputMode::Normal;

            // Only loop in sequence mode
            app.reset_clip();
            app.update_progression()?;
        }
        _ => {}
    }
    Ok(())
}

pub fn status<'a>(app: &App) -> Vec<Span<'a>> {
    let (_, sel_item) = app.selected();

    let span = match sel_item {
        Some(_) => Span::raw("[d]elete [e]dit"),
        None => Span::raw("[a]dd"),
    };
    vec![
        span,
        Span::raw(" loop:[A]-[B] [C]lear [q]:back"),
    ]
}
