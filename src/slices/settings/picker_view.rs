//! Searchable model picker modal for OpenRouter chat and embedding models.
use super::state::{ModelTarget, SettingsState};
use crate::shared::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_model_picker(f: &mut Frame, area: Rect, state: &SettingsState) {
    if !state.picker_active {
        return;
    }

    let popup_area = theme::centered_percent(area, 85, 80);

    f.render_widget(Clear, popup_area);

    let title_target = match state.picker_target {
        ModelTarget::Chat => "Chat / 5D Facet Parse",
        ModelTarget::Embedding => "Embedding / Sémantika",
    };
    let title = format!(" 🔍 Vybrat {} model (OpenRouter) ", title_target);

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Rgb(139, 233, 253))
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::MODAL_BG))
        .border_style(Style::default().fg(Color::Rgb(139, 233, 253)));

    f.render_widget(block, popup_area);

    let inner = popup_area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 1,
    });

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Search input
            Constraint::Min(6),    // Models list
            Constraint::Length(1), // Footer
        ])
        .split(inner);

    // 1. Search Bar
    let filtered = state.filtered_picker_models();
    let search_line = Line::from(vec![
        Span::styled(
            " Hledat: ",
            Style::default()
                .fg(Color::Rgb(139, 233, 253))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if state.picker_search.is_empty() {
                "Všechny modely (napište např. 'qwen', 'claude', 'embed', 'free')..."
            } else {
                &state.picker_search
            },
            if state.picker_search.is_empty() {
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
            } else {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            },
        ),
        Span::styled("█ ", Style::default().fg(Color::Rgb(139, 233, 253))),
        Span::styled(
            format!("({}/{} modelů)", filtered.len(), state.models.len()),
            Style::default().fg(Color::Gray),
        ),
    ]);
    f.render_widget(Paragraph::new(search_line), chunks[0]);

    // 2. Model List Table
    let list_area = chunks[1];
    let max_rows = list_area.height as usize;
    if max_rows <= 1 {
        return;
    }

    let capacity = max_rows.saturating_sub(1);
    let scroll = if state.picker_selected_idx >= capacity {
        state.picker_selected_idx - capacity + 1
    } else {
        0
    };

    let mut lines = Vec::new();

    let total_filtered = filtered.len();
    let pos_indicator = if total_filtered > 0 {
        format!("{}/{}", state.picker_selected_idx + 1, total_filtered)
    } else {
        "-/-".to_string()
    };

    // Table Header
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{:<32}", "Model"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<8}", "Kontext"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<20}", "Cena / 1M tokenů"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:>10}", pos_indicator),
            Style::default().fg(Color::Rgb(139, 233, 253)).add_modifier(Modifier::BOLD),
        ),
    ]));

    if filtered.is_empty() && !state.picker_search.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  ▶ ", Style::default().fg(Color::Yellow).bold()),
            Span::styled("Použít vlastní model ID: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&state.picker_search, Style::default().fg(Color::Cyan).bold()),
            Span::styled("  [Enter pro uložení]", Style::default().fg(Color::DarkGray)),
        ]));
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                "Model nebyl nalezen v OpenRouter katalogu, ale můžete jej zadat přímo.",
                Style::default().fg(Color::DarkGray).italic(),
            ),
        ]));
    } else {
        for (i, m) in filtered.iter().enumerate().skip(scroll).take(capacity) {
            let is_selected = i == state.picker_selected_idx;
            let prefix = if is_selected { "▶ " } else { "  " };

            let name_display = if m.name.len() > 30 {
                format!("{}...", &m.name[..27])
            } else {
                m.name.clone()
            };

            let price_str = m.price_label();
            let price_color = if m.is_free {
                Color::Green
            } else if m.prompt_price_m > 10.0 {
                Color::Yellow
            } else {
                Color::Gray
            };

            let line_style = if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            lines.push(Line::from(vec![
                Span::styled(
                    prefix,
                    if is_selected {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(format!("{:<32}", name_display), line_style),
                Span::styled(
                    format!("{:<8}", m.context_label()),
                    if is_selected {
                        line_style
                    } else {
                        Style::default().fg(Color::Rgb(139, 233, 253))
                    },
                ),
                Span::styled(
                    format!("{:<20}", price_str),
                    if is_selected {
                        line_style
                    } else {
                        Style::default().fg(price_color)
                    },
                ),
            ]));
        }
    }

    f.render_widget(Paragraph::new(lines), list_area);

    // 3. Footer
    let footer_line = Line::from(vec![
        Span::styled(" [↑/↓] ", Style::default().fg(Color::Rgb(139, 233, 253))),
        Span::styled("Vybrat  ", Style::default().fg(Color::Gray)),
        Span::styled("[Enter] ", Style::default().fg(Color::Rgb(139, 233, 253))),
        Span::styled("Potvrdit  ", Style::default().fg(Color::Gray)),
        Span::styled("[Esc] ", Style::default().fg(Color::Rgb(139, 233, 253))),
        Span::styled("Zrušit  ", Style::default().fg(Color::Gray)),
        Span::styled("[r] ", Style::default().fg(Color::Rgb(139, 233, 253))),
        Span::styled("Obnovit katalog", Style::default().fg(Color::Gray)),
    ]);
    f.render_widget(Paragraph::new(footer_line), chunks[2]);
}
