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
    let state = seq.state.lock().unwrap();
    let progression = &state.progression.sequence;
    let bars = state.bars;
    let ticks_per_bar = state.progression.resolution.ticks_per_bar();
    let cur_idx = state.clip_start() + state.tick;

    // The lines that will be rendered.
    let mut lines = vec![];

    for i in 0..bars {
        let mut bars: Vec<Span> = vec![];
        for j in 0..ticks_per_bar {
            let idx = i*ticks_per_bar + j;
            let is_selected = (j, i) == seq.grid_pos;

            bars.push(Span::raw("|"));

            // What character is showing under the cursor
            let tick_char = if progression[idx].is_some() {
                let chord_idx = state.progression.seq_idx_to_chord_idx(idx) + 1;
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
            let (a, b) = state.clip;
            let b = b - 1;
            let is_loop = state.has_loop();
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
    let sel_idx = seq.selected_idx();
    let mut state = seq.state.lock().unwrap();
    let sel_item = &state.progression.sequence[sel_idx];
    let bars = state.progression.bars();
    let ticks_per_bar = state.progression.resolution.ticks_per_bar();

    match key.code {
        // Set the start of the loop
        KeyCode::Char('A') => {
            if state.clip.0 != sel_idx && sel_idx < state.clip.1 {
                state.clip.0 = sel_idx;
            }
        }

        // Set the end of the loop
        KeyCode::Char('B') => {
            let idx = sel_idx + 1;
            if state.clip.1 != idx && sel_idx > state.clip.0 {
                state.clip.1 = idx;
            }
        }

        // Clear the loop
        KeyCode::Char('C') => {
            state.reset_clip();
        }

        // hjkl navigation
        KeyCode::Char('l') => {
            let (x, _) = seq.grid_pos;
            seq.grid_pos.0 = if x >= ticks_per_bar - 1 {
                0
            } else {
                x + 1
            };
        }
        KeyCode::Char('h') => {
            let (x, _) = seq.grid_pos;
            seq.grid_pos.0 = if x == 0 {
                ticks_per_bar - 1
            } else {
                x - 1
            };
        }
        KeyCode::Char('j') => {
            let (_, y) = seq.grid_pos;
            seq.grid_pos.1 = if y >= bars - 1 {
                0
            } else {
                y + 1
            };
        }
        KeyCode::Char('k') => {
            let (_, y) = seq.grid_pos;
            seq.grid_pos.1 = if y == 0 {
                bars - 1
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
        KeyCode::Char('d') => {
            match sel_item {
                None => {},
                Some(_) => {
                    state.progression.delete_chord_at(sel_idx);
                }
            }
        }

        // Select a progression chord by number
        KeyCode::Char(c) => {
            if c.is_numeric() {
                let idx = c.to_string().parse::<usize>()? - 1;
                if let Some(_) = state.progression.chord(idx) {
                    let seq_idx = state.progression.chord_index[idx];

                    let res = state.progression.resolution.ticks_per_bar();
                    let i = seq_idx/res;
                    let j = seq_idx.rem_euclid(res);
                    seq.grid_pos = (j, i);
                }
            }
        }

        _ => {}
    }
    Ok(())
}

pub fn controls<'a>(seq: &Sequencer) -> Vec<Span<'a>> {
    let sel_idx = seq.selected_idx();
    let state = seq.state.lock().unwrap();
    let sel_item = &state.progression.sequence[sel_idx];

    let mut controls = vec![
        Span::raw(" [e]dit"),
    ];
    if sel_item.is_some() {
        controls.push(Span::raw(" [d]elete"));
    }

    controls.push(Span::raw(" loop:[A]-[B]"));
    if state.has_loop() {
        controls.push(Span::raw(" [C]lear"));
    }

    controls
}
