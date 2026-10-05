//! SPAI Note inline edit dialog rendering.
use crate::shared::theme;
use crate::slices::spai_notes::dialog_state::EditField;
use crate::slices::spai_notes::state::SpaiNotesState;
use crate::slices::spai_notes::text_layout;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
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
    let dialog_area = theme::centered_percent(area, 76, 70);
    theme::paint_backdrop(frame, dialog_area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title field
            Constraint::Min(6),    // body field
            Constraint::Length(2), // hotkeys
        ])
        .split(dialog_area);

    let title_focused = state.edit_dialog.field == EditField::Title;
    let body_focused = state.edit_dialog.field == EditField::Body;

    // ── Title field ────────────────────────────────────────────────
    let title_cursor = state
        .edit_dialog
        .cursor
        .min(char_len(&state.edit_dialog.title_input));
    let title_text = if title_focused {
        insert_cursor_marker(&state.edit_dialog.title_input, title_cursor)
    } else {
        state.edit_dialog.title_input.clone()
    };

    let title_inner_w = rows[0].width.saturating_sub(4).max(1) as usize;
    let title_offset = if title_focused && title_cursor >= title_inner_w {
        title_cursor.saturating_sub(title_inner_w.saturating_sub(1))
    } else {
        0
    } as u16;

    let mut title_line = vec![Span::styled(
        "  > ",
        Style::default()
            .fg(if title_focused {
                Color::Yellow
            } else {
                Color::DarkGray
            })
            .bold(),
    )];
    title_line.push(Span::styled(
        title_text,
        Style::default().fg(Color::White).bold(),
    ));

    let title_block = Block::bordered()
        .title(Span::styled(
            theme::field_title("✏️ Název", title_focused),
            Style::default()
                .fg(if title_focused {
                    Color::Yellow
                } else {
                    Color::DarkGray
                })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::field_bg(title_focused)))
        .border_style(Style::default().fg(if title_focused {
            Color::Yellow
        } else {
            Color::DarkGray
        }));
    frame.render_widget(
        Paragraph::new(Line::from(title_line))
            .block(title_block)
            .scroll((0, title_offset)),
        rows[0],
    );

    // ── Body field ─────────────────────────────────────────────────
    let body_cursor = state
        .edit_dialog
        .cursor
        .min(char_len(&state.edit_dialog.body_input));
    let body_text = if body_focused {
        insert_cursor_marker(&state.edit_dialog.body_input, body_cursor)
    } else {
        state.edit_dialog.body_input.clone()
    };

    let body_block = Block::bordered()
        .title(Span::styled(
            theme::field_title("📄 Tělo poznámky", body_focused),
            Style::default()
                .fg(if body_focused {
                    Color::Cyan
                } else {
                    Color::DarkGray
                })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::field_bg(body_focused)))
        .border_style(Style::default().fg(if body_focused {
            Color::Cyan
        } else {
            Color::DarkGray
        }));

    let inner_w = rows[1].width.saturating_sub(2).max(1) as usize;
    let view_h = rows[1].height.saturating_sub(2).max(1) as usize;
    let body_rows = text_layout::wrap_rows(&body_text, inner_w);
    let max_scroll = body_rows.len().saturating_sub(view_h);

    let scroll: u16 = if !body_focused {
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

    let body_lines: Vec<Line> = body_rows.into_iter().map(Line::from).collect();
    let body_para = Paragraph::new(body_lines)
        .block(body_block)
        .scroll((scroll, 0));
    frame.render_widget(body_para, rows[1]);

    // ── Hotkeys ────────────────────────────────────────────────────
    let focus_label = if title_focused { "Název" } else { "Tělo" };
    let hotkeys = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("  [←/→↑/↓]", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" kurzor  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Home/End]", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" řádek  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Del]", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" smazat  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Tab]", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" pole  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Ctrl+S]", Style::default().fg(Color::Green).bold()),
            Span::styled(" uložit  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Ctrl+E]", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" ext  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Esc]", Style::default().fg(Color::Yellow)),
            Span::styled(" zrušit", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  Aktivní pole: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                focus_label,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "   (Enter: název → tělo / nový řádek · PgUp/PgDn posun těla)",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ]);
    frame.render_widget(hotkeys, rows[2]);
}
