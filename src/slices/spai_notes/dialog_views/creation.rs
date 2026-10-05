//! SPAI Smart Input creation dialog — one editor surface (plan §1).
//!
//! Full-page modal with exactly two elements: the editor (input line; while
//! the input is empty the rest of the surface shows the dim type overview)
//! and a two-line footer (selected-type hint + shortcuts). The type list
//! panel, context panel and inline dedup card are gone — dedup lives in the
//! on-demand overlay (`dedup_overlay.rs`).
use super::creation_parts::{build_project_picker_lines, build_shortcuts_line, build_type_footer, build_type_overview};
use super::dedup_overlay::render_dedup_overlay;
use crate::shared::theme;
use crate::slices::spai_notes::input_highlighter::highlight_spai_input_spans;
use crate::slices::spai_notes::state::SpaiNotesState;
use crate::slices::spai_notes::type_options::SPAI_TYPE_OPTIONS;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Wrap},
    Frame,
};

/// Rows of a picker that fit in `box_rows` (borders included), and the scroll
/// offset that keeps `selected` on screen. Pure, so paging is testable
/// without a terminal.
pub(crate) fn picker_viewport(count: usize, selected: usize, box_rows: u16) -> (u16, u16) {
    let count16 = count as u16;
    let visible = box_rows.saturating_sub(2).clamp(1, count16.max(1));
    let sel = (selected as u16).min(count16.saturating_sub(1));
    let scroll = (sel + 1).saturating_sub(visible);
    (visible, scroll)
}

pub fn render_creation_dialog(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    theme::paint_backdrop(frame, area);
    let dialog_area = area;

    let sel_idx = state.creation_dialog.type_selection % SPAI_TYPE_OPTIONS.len();
    let sel_opt = &SPAI_TYPE_OPTIONS[sel_idx];

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(dialog_area);

    // ── Editor block: input line + (empty input → dim overview) ─────
    let header = format!(" ✍ ▸ {} ", sel_opt.name);
    let editor_block = Block::bordered()
        .title(Span::styled(
            header,
            Style::default().fg(sel_opt.color).bold(),
        ))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(sel_opt.color));

    let mut lines = Vec::new();
    lines.push(build_input_line(state));
    if state.creation_dialog.title_input.trim().is_empty() {
        lines.push(Line::raw(""));
        lines.extend(build_type_overview());
    }

    let editor = Paragraph::new(lines)
        .block(editor_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(editor, rows[0]);

    // ── Footer: type hint + shortcuts ──────────────────────────────
    let footer = Paragraph::new(vec![build_type_footer(sel_opt), build_shortcuts_line()]);
    frame.render_widget(footer, rows[1]);

    // ── Autocomplete popup for @project ─────────────────────────────
    if state.creation_dialog.autocomplete_active && !state.creation_dialog.suggestions.is_empty() {
        let count = state.creation_dialog.suggestions.len();
        let box_rows = (dialog_area.height.saturating_sub(10)).clamp(4, 8);
        let selected = state.creation_dialog.autocomplete_selected;
        let (visible, scroll) = picker_viewport(count, selected, box_rows);

        let ac_area = Rect {
            x: dialog_area.x + 4,
            y: dialog_area.y + 2,
            width: dialog_area.width.saturating_sub(8),
            height: visible + 2,
        };

        frame.render_widget(Clear, ac_area);

        let ac_block = Block::bordered()
            .title(format!(
                " 📁 Projekt [{}/{}] [↑/↓, Tab/Enter] ",
                selected + 1,
                count
            ))
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(45, 213, 183)));

        let ac_lines =
            build_project_picker_lines(&state.creation_dialog.suggestions, selected);
        let ac_para = Paragraph::new(ac_lines).block(ac_block).scroll((scroll, 0));
        frame.render_widget(ac_para, ac_area);
    }

    render_dedup_overlay(frame, dialog_area, state);
}

/// The single input line with the block cursor and live SPAI highlighting.
fn build_input_line(state: &SpaiNotesState) -> Line<'static> {
    let raw = state.creation_dialog.title_input.clone();
    let mut spans = vec![Span::styled(
        "  > ",
        Style::default().fg(Color::Yellow).bold(),
    )];

    let chars: Vec<char> = raw.chars().collect();
    let cur = state.creation_dialog.cursor.min(chars.len());

    if raw.is_empty() {
        spans.push(Span::styled(
            " ",
            Style::default().bg(Color::Yellow).fg(Color::Black).bold(),
        ));
        spans.push(Span::styled(
            " Začněte psát — typ = prefix, projekt = @projekt v textu…",
            Style::default().fg(Color::DarkGray),
        ));
        return Line::from(spans);
    }

    let before: String = chars[..cur].iter().collect();
    let at_cursor = if cur < chars.len() { chars[cur] } else { ' ' };
    let after: String = if cur < chars.len() {
        chars[cur + 1..].iter().collect()
    } else {
        String::new()
    };

    if !before.is_empty() {
        spans.extend(highlight_spai_input_spans(&before));
    }
    spans.push(Span::styled(
        at_cursor.to_string(),
        Style::default().bg(Color::Yellow).fg(Color::Black).bold(),
    ));
    if !after.is_empty() {
        spans.extend(highlight_spai_input_spans(&after));
    }

    Line::from(spans)
}


