//! `@` project autocomplete popup for the Scratchpad, rendered **the same
//! way as the smart input's picker** (`n` in the Notes tab): teal rounded
//! box titled `📁 Projekt [n/m] [↑/↓, Tab/Enter]`, `▶ @insert (name)` rows,
//! scrolling viewport. Anchored under the cursor line instead of the dialog
//! top, because the scratchpad line is the context.
use super::super::state::SpaiNotesState;
use super::mention::mention_matches;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
    Frame,
};

pub fn render_mention_popup(frame: &mut Frame, buffer_area: Rect, state: &SpaiNotesState) {
    if state.scratch.mention.is_none() {
        return;
    }
    let matches = mention_matches(&state.scratch, &state.projects);
    if matches.is_empty() {
        return; // the smart input shows nothing until something matches
    }
    let count = matches.len();
    let selected = state
        .scratch
        .mention
        .as_ref()
        .map(|m| m.selected.min(count - 1))
        .unwrap_or(0);

    // Viewport math shared with the smart input's picker (box of 4–8 rows).
    let box_rows = (buffer_area.height.saturating_sub(6)).clamp(4, 8);
    let count16 = count as u16;
    let visible = box_rows.saturating_sub(2).clamp(1, count16.max(1));
    let sel16 = (selected as u16).min(count16.saturating_sub(1));
    let scroll = (sel16 + 1).saturating_sub(visible);

    let width = buffer_area.width.saturating_sub(2).min(56);
    let height = visible + 2;
    if width == 0 || buffer_area.height < height {
        return;
    }

    // Anchor below the cursor line (same rule as the dedup popup).
    let Some(cursor_y) = super::line_render::cursor_visual_row(buffer_area, state) else {
        return;
    };
    let mut y = cursor_y + 1;
    if y + height > buffer_area.y + buffer_area.height {
        y = cursor_y.saturating_sub(height); // above the line
    }
    y = y.max(buffer_area.y);

    let area = Rect::new(buffer_area.x + 1, y, width, height);

    let block = Block::bordered()
        .title(format!(
            " 📁 Projekt [{}/{}] [↑/↓, Tab/Enter] ",
            selected + 1,
            count
        ))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(45, 213, 183)));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    for (i, s) in matches.iter().enumerate() {
        let is_sel = i == selected;
        lines.push(Line::from(vec![
            Span::styled(
                if is_sel { "▶ " } else { "  " },
                if is_sel {
                    Style::default().fg(Color::Yellow).bold()
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(
                s.insert_text.clone(),
                if is_sel {
                    Style::default().fg(Color::Yellow).bold()
                } else {
                    Style::default().fg(Color::Cyan)
                },
            ),
            Span::styled(
                format!(" ({})", s.name),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().add_modifier(Modifier::empty()))
            .scroll((scroll, 0)),
        inner,
    );
}
