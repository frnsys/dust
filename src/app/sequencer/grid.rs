use anyhow::Result;
use tui::{
    layout::Alignment,
    style::{Style, Color},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Borders},
};
use crossterm::event::{KeyEvent, KeyCode};
use super::{Sequencer, InputMode, ChordSelect, ChordTarget};

pub fn render<'a>(seq: &Sequencer) -> Paragraph<'a> {
    let progression = &seq.progression.sequence;
    let resolution = seq.template.resolution;
    let bars = seq.bars;
    let cur_idx = seq.clip_start() + seq.tick - 1;
    let selected = seq.seq_pos;

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
                let chord_idx = seq.progression.seq_idx_to_chord_idx(idx) + 1;
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
            let (a, b) = seq.clip;
            let b = b - 1;
            let is_loop = seq.has_loop();
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

pub fn process_input(seq: &mut Sequencer, key: KeyEvent) -> Result<()> {
    let (sel_idx, sel_item) = seq.selected();

    match key.code {
        // Set the start of the loop
        KeyCode::Char('A') => {
            if seq.clip.0 != sel_idx && sel_idx < seq.clip.1 {
                seq.clip.0 = sel_idx;
                seq.restart_events()?;
            }
        }

        // Set the end of the loop
        KeyCode::Char('B') => {
            let idx = sel_idx + 1;
            if seq.clip.1 != idx && sel_idx > seq.clip.0 {
                seq.clip.1 = idx;
                seq.restart_events()?;
            }
        }

        // Clear the loop
        KeyCode::Char('C') => {
            seq.reset_clip();
            seq.restart_events()?;
        }

        // hjkl navigation
        KeyCode::Char('l') => {
            let (x, _) = seq.seq_pos;
            seq.seq_pos.0 = if x >= seq.template.resolution - 1 {
                0
            } else {
                x + 1
            };
        }
        KeyCode::Char('h') => {
            let (x, _) = seq.seq_pos;
            seq.seq_pos.0 = if x == 0 {
                seq.template.resolution - 1
            } else {
                x - 1
            };
        }
        KeyCode::Char('j') => {
            let (_, y) = seq.seq_pos;
            seq.seq_pos.1 = if y >= seq.progression.bars - 1 {
                0
            } else {
                y + 1
            };
        }
        KeyCode::Char('k') => {
            let (_, y) = seq.seq_pos;
            seq.seq_pos.1 = if y == 0 {
                seq.progression.bars - 1
            } else {
                y - 1
            };
        }

        // Edit or add chord at cursor
        KeyCode::Char('e') => {
            let select = if let Some(cs) = sel_item {
                ChordSelect::with_chord(cs)
            } else {
                ChordSelect::default()
            };
            seq.message = "";
            seq.input_mode = InputMode::Chord(
                select,
                ChordTarget::Chord);
        }

        // Delete chord under cursor
        KeyCode::Char('x') => {
            match sel_item {
                None => {},
                Some(_) => {
                    seq.progression.delete_chord_at(sel_idx);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn controls<'a>(seq: &Sequencer) -> Vec<Span<'a>> {
    let (_, sel_item) = seq.selected();
    let mut controls = vec![
        Span::raw(" [e]dit"),
    ];
    if sel_item.is_some() {
        controls.push(Span::raw(" [x]delete"));
    }

    controls.push(Span::raw(" loop:[A]-[B]"));
    if seq.has_loop() {
        controls.push(Span::raw(" [C]lear"));
    }

    controls
}
