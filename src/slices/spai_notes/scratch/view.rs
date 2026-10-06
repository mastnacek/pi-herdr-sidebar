//! Scratchpad rendering (plan §1, §6): full-area editor, virtualised line
//! window, saved-line visuals (`✓ → project SPAI-014`, dim + italic).
use super::footer::render_footer;
use super::line_model::LineOrigin;
use super::line_render::{cursor_row_y, render_line, HIGHLIGHT_BG};
use super::super::state::SpaiNotesState;
use crate::shared::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

pub fn render_scratch(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    theme::paint_backdrop(frame, area);
    let scratch = &state.scratch;

    let title = format!(
        " 📝 Scratchpad — {}{} ",
        scratch.scope.label(),
        if scratch.dirty { " ●" } else { "" },
    );
    // Yellow frame: the writing colour, matching the `. ` mark highlighting.
    let frame_color = Color::Rgb(255, 215, 0);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    let block = Block::bordered()
        .title(Span::styled(title, Style::default().fg(frame_color).bold()))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(frame_color));
    let inner = block.inner(rows[0]);
    frame.render_widget(block, rows[0]);

    render_lines(frame, inner, state);
    render_footer(frame, rows[1], state);

    // Dedup popup anchored under the cursor line (above the footer).
    super::dedup_popup::render_dedup_popup(frame, inner, state);
    // `@` project autocomplete while a mention token is open.
    super::mention_popup::render_mention_popup(frame, inner, state);

    // Fullscreen `?` help overlay sits above everything else.
    super::help::render_help_overlay(frame, area, state);
}

/// Visible window of lines that keeps the cursor on screen.
pub fn visible_window(total: usize, cursor: usize, height: usize) -> (usize, usize) {
    let height = height.max(1);
    if total <= height {
        return (0, total);
    }
    let scroll = cursor.saturating_sub(height.saturating_sub(1));
    (scroll.min(total.saturating_sub(height)), height)
}

fn render_lines(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    let scratch = &state.scratch;
    let inner_h = area.height as usize;

    // With an active filter, hidden records are skipped entirely — only the
    // matches stay visible (live search). The record under the cursor never
    // hides (you are editing it); visibility is recomputed every frame.
    let shown: Vec<usize> = (0..scratch.lines.len())
        .filter(|&i| i == scratch.cursor_line || !scratch.record_hidden(i))
        .collect();
    let cursor_pos = shown
        .iter()
        .position(|&i| i == scratch.cursor_line)
        .unwrap_or(0);
    let (scroll, _) = visible_window(shown.len(), cursor_pos, inner_h);

    let mut lines: Vec<Line> = Vec::new();

    for &i in shown
        .iter()
        .skip(scroll)
        .take(inner_h)
    {
        lines.push(render_line(&scratch.lines[i]));
    }

    if scratch.lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Pište — řádek se SPAI značkou (. / x ? - # * % …) = záznam; @projekt určí cíl.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  Ctrl+S uloží soubory · Ctrl+D duplicity · F1 nápověda",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // NO wrap: one buffer line = exactly one visual row, so the highlight
    // band and the cursor cell can never shift. Long lines clip; h_scroll
    // keeps the cursor column visible instead.
    let h_scroll = (scratch.cursor_char + 2).saturating_sub(area.width as usize);
    let para = Paragraph::new(lines).scroll((0, h_scroll as u16));
    frame.render_widget(para, area);

    // Full-row highlight of the cursor line, painted directly onto the
    // buffer over the whole row width (Paragraph styles only its text
    // cells; a plain style set here covers the rest of the row too).
    // `cursor_pos` is the position within the (filtered) visible list.
    if let Some(row_y) = cursor_row_y(area, cursor_pos, scroll) {
        let row = Rect::new(area.x, row_y, area.width, 1);
        frame
            .buffer_mut()
            .set_style(row, Style::default().bg(HIGHLIGHT_BG));

        // The cursor: an inverted cell AT the cursor column — it occupies a
        // character space without moving the record text.
        let x = area.x + 2 + (scratch.cursor_char.saturating_sub(h_scroll)) as u16;
        if x < area.x + area.width {
            if let Some(cell) = frame.buffer_mut().cell_mut((x, row_y)) {
                cell.set_style(
                    Style::default()
                        .bg(HIGHLIGHT_BG)
                        .add_modifier(Modifier::REVERSED),
                );
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_keeps_the_cursor_visible() {
        // Short buffer: everything visible.
        assert_eq!(visible_window(5, 0, 10), (0, 5));
        assert_eq!(visible_window(5, 4, 10), (0, 5));

        // Long buffer: the window follows the cursor.
        assert_eq!(visible_window(100, 0, 10), (0, 10));
        assert_eq!(visible_window(100, 50, 10), (41, 10));
        assert_eq!(visible_window(100, 99, 10), (90, 10));
    }
}
