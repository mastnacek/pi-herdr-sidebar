//! Dialog rendering for SPAI notes (creation and inline editing).
use super::dialog_state::EditField;
use super::state::SpaiNotesState;
use super::text_layout;
use crate::shared::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
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

pub fn render_creation_dialog(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    let dialog_area = theme::centered_percent(area, 64, 34);
    theme::paint_backdrop(frame, dialog_area);

    let raw = &state.creation_dialog.title_input;
    let detected = super::input_highlighter::detect_spai_input(raw);

    let header_title = format!(" ✍ SPAI Smart Input: {} ", detected.prefix_label);

    let title_block = Block::bordered()
        .title(Span::styled(
            header_title,
            Style::default().fg(detected.badge_color).bold(),
        ))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(detected.badge_color));

    let mut input_spans = vec![Span::styled(
        "  Vstup: ",
        Style::default().fg(Color::DarkGray),
    )];

    if raw.is_empty() {
        input_spans.push(Span::styled(
            "| napište . úkol, ? nápad, - poznámku, ! prioritu...",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        let highlighted = super::input_highlighter::highlight_spai_input_spans(raw);
        input_spans.extend(highlighted);
        input_spans.push(Span::styled("█", Style::default().fg(Color::Yellow)));
    }

    let lines = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled("  Detekovaný typ: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(
                    "[{} {}]",
                    detected.prefix_glyph.trim(),
                    detected.prefix_label
                ),
                Style::default().fg(detected.badge_color).bold(),
            ),
            Span::styled(
                "   (Syntax: . / /. x z ? - ! @ :tag:)",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::raw(""),
        Line::from(input_spans),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  [Enter]", Style::default().fg(Color::Green).bold()),
            Span::styled(" Uložit  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Tab]", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" Přepnout typ  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Esc]", Style::default().fg(Color::Yellow)),
            Span::styled(" Zrušit", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let para = Paragraph::new(lines).block(title_block);
    frame.render_widget(para, dialog_area);

    if state.creation_dialog.autocomplete_active && !state.creation_dialog.suggestions.is_empty() {
        let count = state.creation_dialog.suggestions.len() as u16;
        let ac_height = (count + 2).min(8);
        let ac_area = Rect {
            x: dialog_area.x + 4,
            y: dialog_area.y + 6,
            width: dialog_area.width.saturating_sub(8),
            height: ac_height,
        };

        frame.render_widget(Clear, ac_area);

        let ac_block = Block::bordered()
            .title(" 📁 Vyberte projekt [@...] [Tab/Enter] ")
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan));

        let mut ac_lines = Vec::new();
        for (i, sug) in state.creation_dialog.suggestions.iter().enumerate() {
            let is_sel = i == state.creation_dialog.autocomplete_selected;
            let marker = if is_sel { "▶ " } else { "  " };
            ac_lines.push(Line::from(vec![
                Span::styled(
                    marker,
                    Style::default().fg(if is_sel {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("{:<18}", sug.insert_text),
                    Style::default()
                        .fg(if is_sel { Color::Cyan } else { Color::White })
                        .add_modifier(if is_sel {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(
                    format!(" {}", sug.path),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        let ac_para = Paragraph::new(ac_lines).block(ac_block);
        frame.render_widget(ac_para, ac_area);
    }
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

    // Keep the title cursor visible on long titles (1 cell per char is a good
    // enough approximation here — titles are single-line Latin/Czech text).
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

    // The body is pre-wrapped (see `text_layout`), so `scroll` is an exact row
    // index and the cursor can be kept inside the viewport without depending on
    // ratatui's unstable rendered-line-info API.
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
