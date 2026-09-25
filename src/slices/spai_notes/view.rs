//! ZEN UI rendering for the SPAI Notes tab.
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Wrap},
    Frame,
};

use super::note::SpaiStatus;
use super::state::SpaiNotesState;

pub fn render_spai_notes_tab(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    if state.projects.is_empty() {
        let empty_msg = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(
                "  Nebyly nalezeny žádné složky docs/spai v registrovaných projektech.",
                Style::default().fg(Color::DarkGray),
            ),
            Line::styled(
                "  Vytvořte v projektu složku docs/spai nebo zkontrolujte ~/.pi/agent/pi-projects-cache.json",
                Style::default().fg(Color::DarkGray),
            ),
        ])
        .block(
            Block::bordered()
                .title(" SPAI Poznámky ")
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(empty_msg, area);
        return;
    }

    // Split area horizontally: Left = Project & Item selector, Right = ZEN Fileviewer
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);

    render_left_pane(frame, main_chunks[0], state);
    render_right_viewer(frame, main_chunks[1], state);

    if state.creation_dialog.active {
        render_creation_dialog(frame, area, state);
    }
}

fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_creation_dialog(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    let dialog_area = centered_rect(area, 64, 34);
    frame.render_widget(Clear, dialog_area);

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

fn render_left_pane(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Project header / switcher
            Constraint::Min(4),    // Items list
            Constraint::Length(1), // Footer hotkeys hint
        ])
        .split(area);

    // 1. Project Switcher Bar
    let proj_name = state
        .projects
        .get(state.selected_project_idx)
        .map(|p| p.name.as_str())
        .unwrap_or("—");

    let is_curr = state
        .projects
        .get(state.selected_project_idx)
        .is_some_and(|p| {
            state
                .current_project_path
                .as_ref()
                .is_some_and(|cp| *cp == p.path)
        });

    let active_badge = if is_curr { " [AKTIVNÍ]" } else { "" };
    let proj_title = format!(
        " Projekt ({}/{}): {}{} ",
        state.selected_project_idx + 1,
        state.projects.len(),
        proj_name,
        active_badge
    );

    let proj_block = Block::bordered()
        .title(Span::styled(
            proj_title,
            Style::default()
                .fg(if is_curr { Color::Cyan } else { Color::White })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if is_curr {
            Color::Cyan
        } else {
            Color::DarkGray
        }));

    let proj_hint = Paragraph::new(Line::from(vec![
        Span::styled(
            "◄ [← / →] projekt  ",
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::styled("[p] aktivní", Style::default().fg(Color::Yellow)),
    ]))
    .block(proj_block);
    frame.render_widget(proj_hint, chunks[0]);

    // 2. Items List
    let items = state.current_items();
    let mut list_lines: Vec<Line> = Vec::new();

    if items.is_empty() {
        list_lines.push(Line::raw(""));
        list_lines.push(Line::styled(
            "  (Žádné poznámky v tomto projektu)",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        for (i, item) in items.iter().enumerate() {
            let is_selected = i == state.selected_item_idx;
            let marker = if is_selected { "▶ " } else { "  " };

            let glyph_color = match item.status {
                SpaiStatus::Done => Color::Rgb(55, 244, 153), // mint
                SpaiStatus::Working => Color::Rgb(241, 252, 121), // yellow
                SpaiStatus::Waiting => Color::Rgb(189, 147, 249), // violet
                SpaiStatus::Cancelled => Color::DarkGray,
                SpaiStatus::Idea => Color::Rgb(255, 121, 198), // pink
                SpaiStatus::Note => Color::Rgb(139, 233, 253), // cyan
                _ => Color::White,
            };

            let title_color = if is_selected {
                Color::Yellow
            } else if item.status == SpaiStatus::Done {
                Color::DarkGray
            } else {
                Color::White
            };

            let title_display = if item.title.len() > 28 {
                format!("{}...", &item.title[..25])
            } else {
                item.title.clone()
            };

            list_lines.push(Line::from(vec![
                Span::styled(
                    marker,
                    Style::default().fg(if is_selected {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("{:<4}", item.status.glyph()),
                    Style::default().fg(glyph_color).bold(),
                ),
                Span::styled(
                    format!("{:<9}", item.id),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    title_display,
                    Style::default()
                        .fg(title_color)
                        .add_modifier(if is_selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
            ]));
        }
    }

    let items_block = Block::bordered()
        .title(format!(" SPAI Položky ({}) ", items.len()))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let items_para = Paragraph::new(list_lines).block(items_block).scroll((0, 0));
    frame.render_widget(items_para, chunks[1]);

    // 3. Hotkeys footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("[←/→]", Style::default().fg(Color::Cyan).bold()),
        Span::styled(" projekt ", Style::default().fg(Color::DarkGray)),
        Span::styled("[↑/↓]", Style::default().fg(Color::Yellow)),
        Span::styled(" posun ", Style::default().fg(Color::DarkGray)),
        Span::styled("[d/u]", Style::default().fg(Color::Magenta).bold()),
        Span::styled(" čtení ", Style::default().fg(Color::DarkGray)),
        Span::styled("[x]", Style::default().fg(Color::Green).bold()),
        Span::styled(" stav ", Style::default().fg(Color::DarkGray)),
        Span::styled("[n]", Style::default().fg(Color::Yellow).bold()),
        Span::styled(" nová ", Style::default().fg(Color::DarkGray)),
        Span::styled("[p]", Style::default().fg(Color::Yellow)),
        Span::styled(" aktivní", Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(footer, chunks[2]);
}

fn render_right_viewer(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    let item_opt = state.selected_item();

    let (title, content_lines) = if let Some(item) = item_opt {
        (
            format!(" {} · {} ", item.id, item.title),
            super::viewer_content::format_viewer_content(item),
        )
    } else {
        (
            " ZEN Prohlížeč ".to_string(),
            vec![
                Line::raw(""),
                Line::styled(
                    "  Vyberte poznámku ze seznamu vlevo.",
                    Style::default().fg(Color::DarkGray),
                ),
            ],
        )
    };

    let viewer_block = Block::bordered()
        .title(Span::styled(
            title,
            Style::default().fg(Color::Green).bold(),
        ))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let viewer = Paragraph::new(content_lines)
        .block(viewer_block)
        .wrap(Wrap { trim: false })
        .scroll((state.viewer_scroll, 0));

    frame.render_widget(viewer, area);
}
