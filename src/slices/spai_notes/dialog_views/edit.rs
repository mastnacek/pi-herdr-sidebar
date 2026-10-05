//! SPAI Note editor rendering — full-page modal (plan §3, 2026-10-05).
//!
//! Two fields on the whole pane: Název (1 row) and Tělo (the rest), plus a
//! two-line footer with the current mode (`-- NORMAL --` / `-- INSERT --`)
//! and contextual shortcuts. Insert shows today's cursor marker; Normal
//! dims the fields and shows no cursor.
use super::dedup_overlay::render_dedup_overlay;
use crate::shared::theme;
use crate::slices::spai_notes::dialog_state::{EditMode, EditField};
use crate::slices::spai_notes::state::SpaiNotesState;
use crate::slices::spai_notes::text_layout;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

/// Inserts the block cursor glyph at a char index (UTF-8 safe).
fn insert_cursor_marker(text: &str, char_idx: usize) -> String {
    let at = text
        .char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    let mut out = String::with_capacity(text.len() + 3);
    out.push_str(&text[..at]);
    out.push('█');
    out.push_str(&text[at..]);
    out
}

fn char_len(text: &str) -> usize {
    text.chars().count()
}

pub fn render_edit_dialog(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    theme::paint_backdrop(frame, area);
    let dialog_area = area;

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title field
            Constraint::Min(6),    // body field
            Constraint::Length(2), // footer: mode + shortcuts
        ])
        .split(dialog_area);

    let insert_mode = state.edit_dialog.mode == EditMode::Insert;
    let title_focused = state.edit_dialog.field == EditField::Title;
    let body_focused = state.edit_dialog.field == EditField::Body;
    // Only Insert owns a visible cursor; Normal is a reading surface.
    let title_cursor = insert_mode && title_focused;
    let body_cursor = insert_mode && body_focused;

    render_title_field(frame, rows[0], state, title_cursor);
    render_body_field(frame, rows[1], state, body_cursor);
    render_footer(frame, rows[2], state);

    render_dedup_overlay(frame, dialog_area, state);
}

fn render_title_field(frame: &mut Frame, area: Rect, state: &SpaiNotesState, has_cursor: bool) {
    let focused = state.edit_dialog.field == EditField::Title;
    let text = if has_cursor {
        insert_cursor_marker(
            &state.edit_dialog.title_input,
            state
                .edit_dialog
                .cursor
                .min(char_len(&state.edit_dialog.title_input)),
        )
    } else {
        state.edit_dialog.title_input.clone()
    };

    let inner_w = area.width.saturating_sub(4).max(1) as usize;
    let cursor_pos = state
        .edit_dialog
        .cursor
        .min(char_len(&state.edit_dialog.title_input));
    let offset = if has_cursor && cursor_pos >= inner_w {
        cursor_pos.saturating_sub(inner_w.saturating_sub(1))
    } else {
        0
    } as u16;

    let title_line = vec![
        Span::styled(
            "  > ",
            Style::default()
                .fg(if focused { Color::Yellow } else { Color::DarkGray })
                .bold(),
        ),
        Span::styled(text, Style::default().fg(Color::White).bold()),
    ];

    let block = Block::bordered()
        .title(Span::styled(
            theme::field_title("✏️ Název", focused),
            Style::default()
                .fg(if focused { Color::Yellow } else { Color::DarkGray })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::field_bg(focused)))
        .border_style(Style::default().fg(if focused {
            Color::Yellow
        } else {
            Color::DarkGray
        }));

    frame.render_widget(
        Paragraph::new(Line::from(title_line))
            .block(block)
            .scroll((0, offset)),
        area,
    );
}

fn render_body_field(frame: &mut Frame, area: Rect, state: &SpaiNotesState, has_cursor: bool) {
    let focused = state.edit_dialog.field == EditField::Body;
    let body_cursor = state
        .edit_dialog
        .cursor
        .min(char_len(&state.edit_dialog.body_input));
    let text = if has_cursor {
        insert_cursor_marker(&state.edit_dialog.body_input, body_cursor)
    } else {
        state.edit_dialog.body_input.clone()
    };

    let block = Block::bordered()
        .title(Span::styled(
            theme::field_title("📄 Tělo poznámky", focused),
            Style::default()
                .fg(if focused { Color::Cyan } else { Color::DarkGray })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::field_bg(focused)))
        .border_style(Style::default().fg(if focused {
            Color::Cyan
        } else {
            Color::DarkGray
        }));

    let inner_w = area.width.saturating_sub(2).max(1) as usize;
    let view_h = area.height.saturating_sub(2).max(1) as usize;
    let body_rows = text_layout::wrap_rows(&text, inner_w);
    let max_scroll = body_rows.len().saturating_sub(view_h);

    let scroll: u16 = if !has_cursor {
        0
    } else if state.edit_dialog.body_follow {
        let prefix: String = state
            .edit_dialog
            .body_input
            .chars()
            .take(body_cursor)
            .collect();
        let cursor_row = text_layout::wrap_rows(&format!("{prefix}█"), inner_w)
            .len()
            .saturating_sub(1);
        if cursor_row >= view_h {
            (cursor_row + 1 - view_h).min(max_scroll)
        } else {
            0
        }
    } else {
        (state.edit_dialog.body_scroll as usize).min(max_scroll)
    } as u16;

    let lines: Vec<Line> = body_rows.into_iter().map(Line::from).collect();
    frame.render_widget(
        Paragraph::new(lines).block(block).scroll((scroll, 0)),
        area,
    );
}

fn render_footer(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    let insert_mode = state.edit_dialog.mode == EditMode::Insert;
    let confirm_discard = state.edit_dialog.confirm_discard && state.edit_dialog.dirty;
    let dirty_marker = if state.edit_dialog.dirty { " ●" } else { "" };

    let mode_line = if confirm_discard {
        Line::from(Span::styled(
            "  Neuložené změny! Esc = zahodit a zavřít · Ctrl+S = uložit",
            Style::default().fg(Color::Red).bold(),
        ))
    } else {
        Line::from(vec![
            Span::styled(
                if insert_mode { " -- INSERT --" } else { " -- NORMAL --" },
                Style::default()
                    .fg(if insert_mode { Color::Green } else { Color::Cyan })
                    .bold(),
            ),
            Span::styled(dirty_marker, Style::default().fg(Color::Yellow)),
            Span::styled(
                if insert_mode {
                    "  Esc = Normal"
                } else {
                    "  i/a/A = psát · Tab = pole · x = stav · q/Esc = zavřít"
                },
                Style::default().fg(Color::DarkGray),
            ),
        ])
    };

    let keys_line = Line::from(vec![
        Span::styled("[Ctrl+S]", Style::default().fg(Color::Green).bold()),
        Span::styled(" uložit  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Ctrl+E]", Style::default().fg(Color::Cyan).bold()),
        Span::styled(" ext  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Ctrl+D]", Style::default().fg(Color::Rgb(255, 184, 108)).bold()),
        Span::styled(" duplicity  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[PgUp/PgDn]", Style::default().fg(Color::Cyan).bold()),
        Span::styled(" posun", Style::default().fg(Color::DarkGray)),
    ]);

    frame.render_widget(Paragraph::new(vec![mode_line, keys_line]), area);
}
