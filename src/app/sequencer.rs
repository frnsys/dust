use anyhow::Result;
use tui::{
    layout::Alignment,
    style::{Style, Color},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Borders},
};
use crossterm::event::KeyCode;
use super::{App, InputMode, InputTarget};

fn has_loop(app: &App) -> bool {
    let (a, b) = app.clip;
    let a_clip = a > 0;
    let b_clip = b < app.progression.sequence.len();
    a_clip || b_clip
}

pub fn render<'a>(app: &App) -> Paragraph<'a> {
    let progression = &app.progression.sequence;
    let resolution = app.template.resolution;
    let bars = app.bars;
    let cur_idx = app.clip_start() + app.tick - 1;
    let selected = app.seq_pos;

    // The lines that will be rendered.
    let mut lines = vec![];

    for i in 0..bars {
        let mut bars: Vec<Span> = vec![];
        for j in 0..resolution {
            let idx = i*resolution + j;
            let is_selected = (j, i) == selected;

            bars.push(Span::raw("|"));

            // What character is showing under the cursor
            let tick_char = if progression[idx].is_some() {
                let chord_idx = app.progression.seq_idx_to_chord_idx(idx) + 1;
                chord_idx.to_string()
            } else if is_selected {
                "*".to_string()
            } else if idx == cur_idx {
                "*".to_string()
            } else {
                " ".to_string()
            };

            // How the cursor position should be styled
            let mut style = Style::default();
            if is_selected {
                style = style.fg(Color::LightBlue);
            } else if idx == cur_idx {
                style = style.fg(Color::Yellow);
            };

            // Highlight loop
            let (a, b) = app.clip;
            let b = b - 1;
            let is_loop = has_loop(app);
            if is_loop && a == idx {
                style = style.bg(Color::DarkGray);
            }
            if is_loop && b == idx {
                style = style.bg(Color::DarkGray);
            }
            if is_loop {
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
                .title("Sequencer")
                .borders(Borders::TOP)
                .style(Style::default())
        )
}

pub fn process_input(app: &mut App, key: KeyCode) -> Result<()> {
    let (sel_idx, sel_item) = app.selected();

    match key {
        // Set the start of the loop
        KeyCode::Char('A') => {
            if app.clip.0 != sel_idx && sel_idx < app.clip.1 {
                app.clip.0 = sel_idx;
                app.update_progression()?;
            }
        }

        // Set the end of the loop
        KeyCode::Char('B') => {
            let idx = sel_idx + 1;
            if app.clip.1 != idx && sel_idx > app.clip.0 {
                app.clip.1 = idx;
                app.update_progression()?;
            }
        }

        // Clear the loop
        KeyCode::Char('C') => {
            app.reset_clip();
            app.update_progression()?;
        }

        // hjkl navigation
        KeyCode::Char('l') => {
            let (x, _) = app.seq_pos;
            app.seq_pos.0 = if x >= app.template.resolution - 1 {
                0
            } else {
                x + 1
            };
        }
        KeyCode::Char('h') => {
            let (x, _) = app.seq_pos;
            app.seq_pos.0 = if x == 0 {
                app.template.resolution - 1
            } else {
                x - 1
            };
        }
        KeyCode::Char('j') => {
            let (_, y) = app.seq_pos;
            app.seq_pos.1 = if y >= app.progression.bars - 1 {
                0
            } else {
                y + 1
            };
        }
        KeyCode::Char('k') => {
            let (_, y) = app.seq_pos;
            app.seq_pos.1 = if y == 0 {
                app.progression.bars - 1
            } else {
                y - 1
            };
        }

        // Edit or add chord at cursor
        KeyCode::Char('e') => {
            if sel_item.is_some() {
                let chord_idx = app.progression.seq_idx_to_chord_idx(sel_idx);
                app.input_mode = InputMode::Text;
                app.input_target = InputTarget::Chord(chord_idx);
                app.input = app.progression.chord(chord_idx).unwrap().to_string();
            } else {
                let chord_idx = app.progression.seq_idx_to_chord_idx(sel_idx);
                let prev_chord = app.progression.prev_chord(chord_idx);
                let cands = app.template.next(prev_chord, &app.key.mode);
                if cands.len() > 0 {
                    app.progression.insert_chord_at(sel_idx, cands[0].clone());
                } else {
                    let chord = app.template.rand_chord_for_mode(&app.key.mode);
                    app.progression.insert_chord_at(sel_idx, chord);
                }
                app.input_mode = InputMode::Text;
                app.input_target = InputTarget::Chord(chord_idx);
                app.input = "".to_string();
            }
        }

        // Delete chord under cursor
        KeyCode::Char('d') => {
            match sel_item {
                None => {},
                Some(_) => {
                    app.progression.delete_chord_at(sel_idx);
                    app.update_progression()?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn controls<'a>(app: &App) -> Vec<Span<'a>> {
    let (_, sel_item) = app.selected();
    let mut controls = vec![
        Span::raw(" [e]dit"),
    ];
    if sel_item.is_some() {
        controls.push(Span::raw(" [d]elete"));
    }

    controls.push(Span::raw(" loop:[A]-[B]"));
    if has_loop(app) {
        controls.push(Span::raw(" [C]lear"));
    }

    controls
}
