//! Rendering for S.P.A.I. Settings & Vectorization tab.
use super::state::{SettingsField, SettingsState};
use crate::shared::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

fn mask_key(key: &str) -> String {
    if key.is_empty() {
        return "Nenastaveno (zadejte API klíč nebo OPENROUTER_API_KEY)".to_string();
    }
    if key.len() <= 8 {
        return "••••••••".to_string();
    }
    format!("{}••••••••{}", &key[..6], &key[key.len() - 4..])
}

pub fn render_settings_tab(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // API key field
            Constraint::Length(4), // Model selector
            Constraint::Length(5), // Vectorization & Dedup index
            Constraint::Length(3), // Threshold slider
            Constraint::Min(2),    // Status message / info
            Constraint::Length(1), // Footer hotkeys
        ])
        .split(area);

    // ── 1. API Key Card ─────────────────────────────────────────────
    let is_key_sel = state.selected_field == SettingsField::ApiKey;
    let key_block = Block::bordered()
        .title(Span::styled(
            theme::field_title("🔑 OpenRouter API Klíč", is_key_sel),
            Style::default()
                .fg(if is_key_sel {
                    Color::Yellow
                } else {
                    Color::White
                })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::field_bg(is_key_sel)))
        .border_style(Style::default().fg(if is_key_sel {
            Color::Yellow
        } else {
            Color::DarkGray
        }));

    let key_text = if state.editing_api_key {
        format!("  > {}█", state.api_key_input)
    } else {
        format!("  > {}  [e: upravit]", mask_key(&state.api_key))
    };

    let key_para = Paragraph::new(Line::from(vec![Span::styled(
        key_text,
        Style::default().fg(if state.editing_api_key {
            Color::Yellow
        } else {
            Color::White
        }),
    )]))
    .block(key_block);
    frame.render_widget(key_para, rows[0]);

    // ── 2. Models Selection Card ────────────────────────────────────
    let is_chat_sel = state.selected_field == SettingsField::ChatModel;
    let is_embed_sel = state.selected_field == SettingsField::EmbeddingModel;
    let is_models_focused = is_chat_sel || is_embed_sel;

    let models_block = Block::bordered()
        .title(Span::styled(
            theme::field_title("🤖 AI Modely (OpenRouter)", is_models_focused),
            Style::default()
                .fg(if is_models_focused {
                    Color::Yellow
                } else {
                    Color::White
                })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::field_bg(is_models_focused)))
        .border_style(Style::default().fg(if is_models_focused {
            Color::Yellow
        } else {
            Color::DarkGray
        }));

    let models_lines = vec![
        Line::from(vec![
            Span::styled(
                if is_chat_sel { "  ▶ " } else { "    " },
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::styled("Parse / Chat model: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("◄ {} ►", state.current_chat_model()),
                Style::default()
                    .fg(if is_chat_sel {
                        Color::Rgb(45, 213, 183)
                    } else {
                        Color::White
                    })
                    .add_modifier(if is_chat_sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                if is_embed_sel { "  ▶ " } else { "    " },
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::styled("Embedding model:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("◄ {} ►", state.current_embedding_model()),
                Style::default()
                    .fg(if is_embed_sel {
                        Color::Rgb(255, 94, 219)
                    } else {
                        Color::White
                    })
                    .add_modifier(if is_embed_sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ]),
    ];
    let models_para = Paragraph::new(models_lines).block(models_block);
    frame.render_widget(models_para, rows[1]);

    // ── 3. Vectorization & Dedup Index Card ─────────────────────────
    let is_vec_sel = state.selected_field == SettingsField::VectorizeAction;
    let is_cls_sel = state.selected_field == SettingsField::ClassifyAction;
    let is_actions_focused = is_vec_sel || is_cls_sel;

    let vec_block = Block::bordered()
        .title(Span::styled(
            theme::field_title("⚡ Vektorizace & Sémantický Index", is_actions_focused),
            Style::default()
                .fg(if is_actions_focused {
                    Color::Yellow
                } else {
                    Color::White
                })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::field_bg(is_actions_focused)))
        .border_style(Style::default().fg(if is_actions_focused {
            Color::Yellow
        } else {
            Color::DarkGray
        }));

    let vec_lines = vec![
        Line::from(vec![
            Span::styled("    Stav indexu: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(
                    "{} záznamů připraveno k sémantickému prohledávání",
                    state.total_records
                ),
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                if is_vec_sel { "  ▶ " } else { "    " },
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::styled(
                "[v] Spustit vektorizaci všech záznamů (Re-embed all)",
                Style::default()
                    .fg(if is_vec_sel {
                        Color::Rgb(55, 244, 153)
                    } else {
                        Color::Rgb(45, 213, 183)
                    })
                    .add_modifier(if is_vec_sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                if is_cls_sel { "  ▶ " } else { "    " },
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::styled(
                "[f] Spustit 5D AI klasifikaci facetů (area/effort/urgency/who)",
                Style::default()
                    .fg(if is_cls_sel {
                        Color::Rgb(255, 121, 198)
                    } else {
                        Color::Rgb(186, 85, 211)
                    })
                    .add_modifier(if is_cls_sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ]),
    ];
    let vec_para = Paragraph::new(vec_lines).block(vec_block);
    frame.render_widget(vec_para, rows[2]);

    // ── 4. Similarity Threshold Card ────────────────────────────────
    let is_thresh_sel = state.selected_field == SettingsField::SimilarityThreshold;
    let thresh_block = Block::bordered()
        .title(Span::styled(
            theme::field_title("🎯 Práh deduplikace (Similarity Threshold)", is_thresh_sel),
            Style::default()
                .fg(if is_thresh_sel {
                    Color::Yellow
                } else {
                    Color::White
                })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::field_bg(is_thresh_sel)))
        .border_style(Style::default().fg(if is_thresh_sel {
            Color::Yellow
        } else {
            Color::DarkGray
        }));

    let bar = theme::usage_bar(state.similarity_threshold as u64, 100, 20);
    let thresh_line = Line::from(vec![
        Span::styled(
            if is_thresh_sel { "  ▶ " } else { "    " },
            Style::default().fg(Color::Yellow).bold(),
        ),
        Span::styled(
            format!("Práh shody: {}% ", state.similarity_threshold),
            Style::default().fg(Color::White).bold(),
        ),
        Span::styled(
            format!("[{bar}] "),
            Style::default().fg(Color::Rgb(45, 213, 183)),
        ),
        Span::styled("[+/- pro změnu]", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(thresh_line).block(thresh_block), rows[3]);

    // ── 5. Status / Info ────────────────────────────────────────────
    if let Some(msg) = &state.status_message {
        let msg_line = Line::from(vec![
            Span::styled("  ℹ️  ", Style::default().fg(Color::Rgb(45, 213, 183))),
            Span::styled(msg, Style::default().fg(Color::White).bold()),
        ]);
        frame.render_widget(Paragraph::new(msg_line), rows[4]);
    }

    // ── 6. Footer Hotkeys ───────────────────────────────────────────
    let footer_line = Line::from(vec![
        Span::styled("  [↑/↓]", Style::default().fg(Color::Yellow).bold()),
        Span::styled(" Vybrat pole   ", Style::default().fg(Color::DarkGray)),
        Span::styled("[←/→]", Style::default().fg(Color::Yellow).bold()),
        Span::styled(" Změnit model   ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Enter]", Style::default().fg(Color::Rgb(55, 244, 153)).bold()),
        Span::styled(" Spustit / Uložit   ", Style::default().fg(Color::DarkGray)),
        Span::styled("[v]", Style::default().fg(Color::Rgb(45, 213, 183)).bold()),
        Span::styled(" Vektorizovat   ", Style::default().fg(Color::DarkGray)),
        Span::styled("[f]", Style::default().fg(Color::Rgb(255, 121, 198)).bold()),
        Span::styled(" Facety", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(footer_line), rows[5]);
}
