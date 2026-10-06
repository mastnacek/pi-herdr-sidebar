//! Per-line rendering for the Scratchpad: no-wrap Paragraph rows, the full-row
//! cursor highlight and the inverted-cell cursor.
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::line_model::ScratchLine;
use super::super::state::SpaiNotesState;

/// The cursor-line highlight background (dark amber — readable with every
/// fg colour the highlighter uses, distinct from the yellow frame).
pub const HIGHLIGHT_BG: Color = Color::Rgb(58, 48, 8);

/// Buffer y of the cursor inside this area, or None when off-screen.
/// `cursor_pos` is the cursor's position within the (filtered) visible list.
pub fn cursor_row_y(area: Rect, cursor_pos: usize, scroll: usize) -> Option<u16> {
    let rel = cursor_pos.checked_sub(scroll)?;
    if rel < area.height as usize {
        Some(area.y + rel as u16)
    } else {
        None
    }
}

/// Visual row (buffer y) of the cursor inside a rendered scratch area —
/// used by the popups to anchor under the cursor line, accounting for
/// hidden (filtered-out) records.
pub fn cursor_visual_row(area: Rect, state: &SpaiNotesState) -> Option<u16> {
    let scratch = &state.scratch;
    let shown: Vec<usize> = (0..scratch.lines.len())
        .filter(|&i| i == scratch.cursor_line || !scratch.record_hidden(i))
        .collect();
    let cursor_pos = shown
        .iter()
        .position(|&i| i == scratch.cursor_line)
        .unwrap_or(0);
    let inner_h = area.height as usize;
    let (scroll, _) = super::view::visible_window(shown.len(), cursor_pos, inner_h);
    cursor_row_y(area, cursor_pos, scroll)
}

/// One buffer line: every line renders in normal highlighting (records stay
/// fully workable after saving/loading); a record with a file gets a dim
/// `✓ → project id` label on its head only. Done (`x`) records keep their
/// struck-through styling via the highlighter's mark handling.
/// Renders one buffer line. `is_cursor`/`cursor_char` are unused here — the
/// cursor is an inverted buffer cell painted by `render_lines` after the
/// Paragraph, so it occupies a character space WITHOUT shifting the text.
pub fn render_line(line: &ScratchLine) -> Line<'static> {
    let _ = ();
    let mut spans: Vec<Span> = Vec::new();
    let text = line.text.clone();

    spans.push(Span::styled("  ", Style::default()));
    spans.extend(highlight_spai_input_spans(&text));

    // The ✓ label belongs to the record head (the marked line); continuation
    // lines of the same file stay label-free.
    if let Some(origin) = line.origin.file_origin() {
        if super::line_model::is_marked(&line.text) {
            let label = format!(
                "  ✓ → {} {}",
                origin.project().unwrap_or(""),
                origin.record_id().unwrap_or("")
            );
            spans.push(Span::styled(
                label,
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    Line::from(spans)
}


fn highlight_spai_input_spans(text: &str) -> Vec<Span<'static>> {
    super::super::input_highlighter::highlight_spai_input_spans(text)
}
