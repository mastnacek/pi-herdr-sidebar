//! SPAI Smart Input creation dialog rendering with full width, multiline wrapped input, and clean context.
use super::creation_parts::{build_hint_lines, build_project_picker_lines, build_shortcuts_line};
use crate::shared::theme;
use crate::slices::spai_notes::state::SpaiNotesState;
use crate::slices::spai_notes::type_options::SPAI_TYPE_OPTIONS;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Wrap},
    Frame,
};

/// Rows of the project picker that fit in `box_rows` (borders included), and the
/// scroll offset that keeps `selected` on screen. Pure, so paging is testable
/// without a terminal: the popup scrolls instead of clipping the tail.
pub(crate) fn picker_viewport(count: usize, selected: usize, box_rows: u16) -> (u16, u16) {
    let count16 = count as u16;
    let visible = box_rows.saturating_sub(2).clamp(1, count16.max(1));
    let sel = (selected as u16).min(count16.saturating_sub(1));
    let scroll = (sel + 1).saturating_sub(visible);
    (visible, scroll)
}

pub fn render_creation_dialog(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    // Full width of the pane / sidebar window
    let dialog_width = area.width;
    let dialog_height = area.height.clamp(18, 36);
    let x = area.x;
    let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

    theme::paint_backdrop(frame, dialog_area);

    let sel_idx = state.creation_dialog.type_selection % SPAI_TYPE_OPTIONS.len();
    let sel_opt = &SPAI_TYPE_OPTIONS[sel_idx];

    let header_title = format!(" ✍ SPAI Smart Input: {} ", sel_opt.name);

    let title_block = Block::bordered()
        .title(Span::styled(
            header_title,
            Style::default().fg(sel_opt.color).bold(),
        ))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(sel_opt.color));

    let inner = title_block.inner(dialog_area);
    frame.render_widget(title_block, dialog_area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // 1. Top row: Smart Input Box (3 visible lines + borders)
            Constraint::Min(8),    // 2. Middle row: Type list (left) + Context & Dedup (right)
            Constraint::Length(1), // 3. Bottom row: Shortcuts footer
        ])
        .split(inner);

    let raw = &state.creation_dialog.title_input;

    // ── 1. Top row: Vstup (Full-width Multiline Smart Input Box with in-text Cursor) ──
    let input_block = Block::bordered()
        .title(Span::styled(
            " ▶ Zadání (Smart Input) ",
            Style::default().fg(Color::Yellow).bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::FIELD_BG_ACTIVE))
        .border_style(Style::default().fg(Color::Yellow));

    let mut input_spans = vec![Span::styled(
        "  > ",
        Style::default().fg(Color::Yellow).bold(),
    )];

    let chars: Vec<char> = raw.chars().collect();
    let cur = state.creation_dialog.cursor.min(chars.len());

    if raw.is_empty() {
        input_spans.push(Span::styled(
            " ",
            Style::default().bg(Color::Yellow).fg(Color::Black).bold(),
        ));
        input_spans.push(Span::styled(
            format!(
                " [Tab: vložit {}] nebo začněte psát (@projekt, !priorita, :tagy:, @termín)...",
                sel_opt.symbol
            ),
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        let before: String = chars[..cur].iter().collect();
        let at_cursor = if cur < chars.len() { chars[cur] } else { ' ' };
        let after: String = if cur < chars.len() {
            chars[cur + 1..].iter().collect()
        } else {
            String::new()
        };

        if !before.is_empty() {
            let highlighted =
                crate::slices::spai_notes::input_highlighter::highlight_spai_input_spans(&before);
            input_spans.extend(highlighted);
        }

        input_spans.push(Span::styled(
            at_cursor.to_string(),
            Style::default().bg(Color::Yellow).fg(Color::Black).bold(),
        ));

        if !after.is_empty() {
            let highlighted =
                crate::slices::spai_notes::input_highlighter::highlight_spai_input_spans(&after);
            input_spans.extend(highlighted);
        }
    }

    let input_para = Paragraph::new(Line::from(input_spans))
        .block(input_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(input_para, rows[0]);

    // ── 2. Middle row: Type list (left) and Context & Dedup (right) ─
    let mid_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24), // type list
            Constraint::Min(25),    // context & dedup details
        ])
        .split(rows[1]);

    // ── Left column: Typ položky ────────────────────────────────────
    let list_block = Block::bordered()
        .title(Span::styled(
            " Typ záznamu ",
            Style::default().fg(Color::White).bold(),
        ))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let mut type_lines = Vec::new();
    for (i, opt) in SPAI_TYPE_OPTIONS.iter().enumerate() {
        let is_sel = i == sel_idx;
        let marker = if is_sel { "▸ " } else { "  " };
        let line = Line::from(vec![
            Span::styled(
                marker,
                Style::default().fg(if is_sel {
                    opt.color
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                format!("{:<3}", opt.display_sym),
                Style::default().fg(opt.color).bold(),
            ),
            Span::styled(
                format!(" {}", opt.name),
                Style::default()
                    .fg(if is_sel {
                        Color::White
                    } else {
                        Color::DarkGray
                    })
                    .add_modifier(if is_sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ]);
        type_lines.push(line);
    }
    let (_visible, scroll) = picker_viewport(SPAI_TYPE_OPTIONS.len(), sel_idx, mid_cols[0].height);
    let type_para = Paragraph::new(type_lines)
        .block(list_block)
        .scroll((scroll, 0));
    frame.render_widget(type_para, mid_cols[0]);

    // ── Right column: Nápověda & Dedup ──────────────────────────────
    let hint_block = Block::bordered()
        .title(Span::styled(
            format!(" Kontext: {} ", sel_opt.name),
            Style::default().fg(sel_opt.color).bold(),
        ))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(sel_opt.color));

    let hint_lines = build_hint_lines(
        sel_opt,
        raw,
        state.current_items(),
        &state.creation_dialog.debounced_matches,
        state.creation_dialog.is_evaluating_vector,
        state.creation_dialog.last_keystroke,
    );
    let hint_para = Paragraph::new(hint_lines).block(hint_block);
    frame.render_widget(hint_para, mid_cols[1]);

    // ── 3. Bottom row: Klávesové zkratky ────────────────────────────
    let shortcuts_line = build_shortcuts_line();
    frame.render_widget(Paragraph::new(shortcuts_line), rows[2]);

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
}
