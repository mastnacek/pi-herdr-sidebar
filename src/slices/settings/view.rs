//! Settings tab Ratatui UI rendering.
use super::cards_view::{render_key_card, render_models_card};
use super::picker_view::render_model_picker;
use super::state::{SettingsField, SettingsState};
use crate::shared::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

pub fn render_settings_tab(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // 1. API Key Card
            Constraint::Length(7), // 2. Model Selection Card
            Constraint::Length(5), // 3. Vectorization & Actions Card
            Constraint::Length(4), // 4. Similarity Threshold Slider Card
            Constraint::Min(2),    // 5. Status message / Info
            Constraint::Length(2), // 6. Footer Hotkeys
        ])
        .split(area);

    // ── 1. API Key Card ─────────────────────────────────────────────
    render_key_card(frame, rows[0], state);

    // ── 2. Models Selection Card ────────────────────────────────────
    render_models_card(frame, rows[1], state);

    // ── 3. Vectorization & Dedup Index Card ─────────────────────────
    let is_vec_sel = state.selected_field == SettingsField::VectorizeAction;
    let is_cls_sel = state.selected_field == SettingsField::ClassifyAction;
    let is_actions_focused = is_vec_sel || is_cls_sel;

    let vec_block = Block::bordered()
        .title(Span::styled(
            theme::field_title("⚡ Vektorizace & Sémantický Index", is_actions_focused),
            Style::default()
                .fg(if is_actions_focused { Color::Yellow } else { Color::White })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::field_bg(is_actions_focused)))
        .border_style(Style::default().fg(if is_actions_focused {
            Color::Yellow
        } else {
            Color::DarkGray
        }));

    let mut vec_lines = Vec::new();
    if state.is_busy {
        let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let spinner = spinner_frames[state.spinner_tick % spinner_frames.len()];

        let pct = if state.busy_total > 0 {
            (state.busy_step * 100) / state.busy_total
        } else {
            0
        };
        let bar_len = 16;
        let fill = (pct * bar_len) / 100;
        let prog_bar = format!("{}{}", "█".repeat(fill), "░".repeat(bar_len.saturating_sub(fill)));

        vec_lines.push(Line::from(vec![
            Span::styled(format!("  {} ", spinner), Style::default().fg(Color::Yellow).bold()),
            Span::styled(&state.busy_label, Style::default().fg(Color::Yellow).bold()),
        ]));
        vec_lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(format!("[{}] ", prog_bar), Style::default().fg(Color::Rgb(45, 213, 183))),
            Span::styled(format!("{}/{} ({}%)", state.busy_step, state.busy_total, pct), Style::default().fg(Color::White)),
        ]));
        vec_lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled("Probíhá na pozadí — TUI zůstává plně interaktivní", Style::default().fg(Color::DarkGray).italic()),
        ]));
    } else {
        vec_lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled("Stav indexu: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(
                    "{} záznamů připraveno k sémantickému prohledávání",
                    state.total_records
                ),
                Style::default().fg(Color::White),
            ),
        ]));
        vec_lines.push(Line::from(vec![
            Span::styled(
                if is_vec_sel { "  ▶ " } else { "    " },
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::styled(
                "[v] Spustit vektorizaci všech záznamů (Re-embed all)",
                Style::default()
                    .fg(if is_vec_sel { Color::Rgb(45, 213, 183) } else { Color::White })
                    .add_modifier(if is_vec_sel { Modifier::BOLD } else { Modifier::empty() }),
            ),
        ]));
        vec_lines.push(Line::from(vec![
            Span::styled(
                if is_cls_sel { "  ▶ " } else { "    " },
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::styled(
                "[f] Spustit 5D AI klasifikaci facetů (area/effort/urgency/who)",
                Style::default()
                    .fg(if is_cls_sel { Color::Rgb(255, 94, 219) } else { Color::White })
                    .add_modifier(if is_cls_sel { Modifier::BOLD } else { Modifier::empty() }),
            ),
        ]));
    }
    let vec_para = Paragraph::new(vec_lines).block(vec_block);
    frame.render_widget(vec_para, rows[2]);

    // ── 4. Similarity Threshold Slider Card ─────────────────────────
    let is_thresh_sel = state.selected_field == SettingsField::SimilarityThreshold;
    let thresh_block = Block::bordered()
        .title(Span::styled(
            theme::field_title("🎯 Práh deduplikace (Similarity Threshold)", is_thresh_sel),
            Style::default()
                .fg(if is_thresh_sel { Color::Yellow } else { Color::White })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::field_bg(is_thresh_sel)))
        .border_style(Style::default().fg(if is_thresh_sel {
            Color::Yellow
        } else {
            Color::DarkGray
        }));

    let bar_len = 20;
    let filled_len = (state.similarity_threshold as usize * bar_len) / 100;
    let empty_len = bar_len.saturating_sub(filled_len);
    let bar_str = format!("{}{}", "█".repeat(filled_len), "░".repeat(empty_len));

    let thresh_lines = vec![Line::from(vec![
        Span::styled(
            if is_thresh_sel { "  ▶ " } else { "    " },
            Style::default().fg(Color::Yellow).bold(),
        ),
        Span::styled("Práh shody: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}% ", state.similarity_threshold),
            Style::default().fg(Color::Yellow).bold(),
        ),
        Span::styled(
            format!("[{}] ", bar_str),
            Style::default().fg(Color::Rgb(45, 213, 183)),
        ),
        Span::styled(
            "[+/- nebo ←/→ pro změnu]",
            Style::default().fg(Color::DarkGray),
        ),
    ])];
    let thresh_para = Paragraph::new(thresh_lines).block(thresh_block);
    frame.render_widget(thresh_para, rows[3]);

    // ── 5. Status line ──────────────────────────────────────────────
    if let Some(msg) = &state.status_message {
        let status_line = Line::from(vec![
            Span::styled("  ℹ ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(msg, Style::default().fg(Color::White)),
        ]);
        frame.render_widget(Paragraph::new(status_line), rows[4]);
    }

    // ── 6. Footer Hotkeys ───────────────────────────────────────────
    let footer_line = Line::from(vec![
        Span::styled(" [↑/↓] ", Style::default().fg(Color::DarkGray)),
        Span::styled("Vybrat pole   ", Style::default().fg(Color::White)),
        Span::styled("[←/→] ", Style::default().fg(Color::DarkGray)),
        Span::styled("Změnit model   ", Style::default().fg(Color::White)),
        Span::styled("[Enter] ", Style::default().fg(Color::DarkGray)),
        Span::styled("Hledat/Spustit   ", Style::default().fg(Color::White)),
        Span::styled("[r] ", Style::default().fg(Color::DarkGray)),
        Span::styled("Obnovit katalog", Style::default().fg(Color::White)),
    ]);
    frame.render_widget(Paragraph::new(footer_line), rows[5]);

    // ── Modal Popup (if active) ─────────────────────────────────────
    render_model_picker(frame, area, state);
}
