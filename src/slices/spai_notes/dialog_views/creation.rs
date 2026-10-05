//! SPAI Smart Input creation dialog rendering.
use crate::shared::theme;
use crate::slices::spai_notes::state::SpaiNotesState;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
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
    let dialog_width = area.width.saturating_sub(6).clamp(52, 70);
    let dialog_height = 7u16.min(area.height.saturating_sub(2)).max(5);
    let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
    let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

    theme::paint_backdrop(frame, dialog_area);

    let raw = &state.creation_dialog.title_input;
    let detected = crate::slices::spai_notes::input_highlighter::detect_spai_input(raw);

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
        Style::default().fg(Color::Rgb(193, 196, 151)),
    )];

    if raw.is_empty() {
        input_spans.push(Span::styled(
            "|",
            Style::default().fg(Color::Rgb(241, 252, 121)).bold(),
        ));
        input_spans.push(Span::styled(" napište ", Style::default().fg(Color::DarkGray)));
        input_spans.push(Span::styled(". ", Style::default().fg(Color::Rgb(241, 252, 121)).bold()));
        input_spans.push(Span::styled("úkol, ", Style::default().fg(Color::DarkGray)));
        input_spans.push(Span::styled("? ", Style::default().fg(Color::Rgb(255, 121, 198)).bold()));
        input_spans.push(Span::styled("nápad, ", Style::default().fg(Color::DarkGray)));
        input_spans.push(Span::styled("- ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()));
        input_spans.push(Span::styled("poznámku, ", Style::default().fg(Color::DarkGray)));
        input_spans.push(Span::styled("! ", Style::default().fg(Color::Rgb(255, 83, 69)).bold()));
        input_spans.push(Span::styled("priorit", Style::default().fg(Color::DarkGray)));
    } else {
        let highlighted = crate::slices::spai_notes::input_highlighter::highlight_spai_input_spans(raw);
        input_spans.extend(highlighted);
        input_spans.push(Span::styled(
            "█",
            Style::default().fg(Color::Rgb(241, 252, 121)),
        ));
    }

    let lines = vec![
        Line::from(vec![
            Span::styled("  Detekovaný typ: ", Style::default().fg(Color::Rgb(193, 196, 151))),
            Span::styled(
                format!(
                    "[{} {}]   ",
                    detected.prefix_glyph.trim(),
                    detected.short_label
                ),
                Style::default().fg(detected.badge_color).bold(),
            ),
            Span::styled("(Syntax: ", Style::default().fg(Color::DarkGray)),
            Span::styled(". ", Style::default().fg(Color::Rgb(241, 252, 121)).bold()),
            Span::styled("/ ", Style::default().fg(Color::Rgb(241, 252, 121)).bold()),
            Span::styled("/. ", Style::default().fg(Color::Rgb(189, 147, 249)).bold()),
            Span::styled("x ", Style::default().fg(Color::Rgb(55, 244, 153)).bold()),
            Span::styled("z ", Style::default().fg(Color::Rgb(135, 145, 170)).bold()),
            Span::styled("? ", Style::default().fg(Color::Rgb(255, 121, 198)).bold()),
            Span::styled("- ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
            Span::styled("! ", Style::default().fg(Color::Rgb(255, 83, 69)).bold()),
            Span::styled("@ ", Style::default().fg(Color::Rgb(45, 213, 183)).bold()),
            Span::styled(":tag:", Style::default().fg(Color::Rgb(55, 244, 153))),
            Span::styled(")", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(input_spans),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  [Enter]", Style::default().fg(Color::Rgb(55, 244, 153)).bold()),
            Span::styled(" Uložit   ", Style::default().fg(Color::Rgb(193, 196, 151))),
            Span::styled("[Tab]", Style::default().fg(Color::Rgb(45, 213, 183)).bold()),
            Span::styled(" Přepnout typ   ", Style::default().fg(Color::Rgb(193, 196, 151))),
            Span::styled("[Esc]", Style::default().fg(Color::Rgb(241, 252, 121)).bold()),
            Span::styled(" Zrušit", Style::default().fg(Color::Rgb(193, 196, 151))),
        ]),
    ];

    let para = Paragraph::new(lines).block(title_block);
    frame.render_widget(para, dialog_area);

    if state.creation_dialog.autocomplete_active && !state.creation_dialog.suggestions.is_empty() {
        let count = state.creation_dialog.suggestions.len();
        let list_y = dialog_area.bottom();
        let box_rows = area.bottom().saturating_sub(list_y).clamp(3, 12);
        let selected = state.creation_dialog.autocomplete_selected;
        let (visible, scroll) = picker_viewport(count, selected, box_rows);

        let ac_area = Rect {
            x: dialog_area.x + 2,
            y: list_y.min(area.bottom().saturating_sub(visible + 2)),
            width: dialog_area.width.saturating_sub(4),
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

        let mut ac_lines = Vec::new();
        for (i, sug) in state.creation_dialog.suggestions.iter().enumerate() {
            let is_sel = i == state.creation_dialog.autocomplete_selected;
            let marker = if is_sel { "▶ " } else { "  " };
            ac_lines.push(Line::from(vec![
                Span::styled(
                    marker,
                    Style::default().fg(if is_sel {
                        Color::Rgb(241, 252, 121)
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("{:<18}", sug.insert_text),
                    Style::default()
                        .fg(if is_sel {
                            Color::Rgb(45, 213, 183)
                        } else {
                            Color::White
                        })
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

        let ac_para = Paragraph::new(ac_lines).block(ac_block).scroll((scroll, 0));
        frame.render_widget(ac_para, ac_area);
    }
}
