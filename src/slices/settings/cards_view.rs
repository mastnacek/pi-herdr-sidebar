//! Individual card renderers for the settings tab.
use super::state::{SettingsField, SettingsState};
use crate::shared::theme;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

pub fn mask_key(key: &str) -> String {
    if key.is_empty() {
        "Není nastaven (vyžadováno pro OpenRouter AI)".to_string()
    } else if key.len() <= 12 {
        "••••••••••••".to_string()
    } else {
        format!("{}••••••••••••{}", &key[..8], &key[key.len() - 4..])
    }
}

pub fn render_key_card(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let is_key_sel = state.selected_field == SettingsField::ApiKey;
    let key_block = Block::bordered()
        .title(Span::styled(
            theme::field_title("🔑 OpenRouter API Klíč", is_key_sel),
            Style::default()
                .fg(if is_key_sel { Color::Yellow } else { Color::White })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::field_bg(is_key_sel)))
        .border_style(Style::default().fg(if is_key_sel { Color::Yellow } else { Color::DarkGray }));

    let key_text = if state.editing_api_key {
        format!("  > {}█", state.api_key_input)
    } else {
        format!("  > {}  [e: upravit]", mask_key(&state.api_key))
    };

    let key_para = Paragraph::new(Line::from(vec![Span::styled(
        key_text,
        Style::default().fg(if state.editing_api_key { Color::Yellow } else { Color::White }),
    )]))
    .block(key_block);
    frame.render_widget(key_para, area);
}

pub fn render_models_card(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let is_chat_sel = state.selected_field == SettingsField::ChatModel;
    let is_embed_sel = state.selected_field == SettingsField::EmbeddingModel;
    let is_models_focused = is_chat_sel || is_embed_sel;

    let chat_info = state.current_chat_info();
    let embed_info = state.current_embedding_info();

    let chat_price = chat_info
        .map(|m| format!("{} ({})", m.price_label(), m.context_label()))
        .unwrap_or_else(|| "-".to_string());
    let embed_price = embed_info
        .map(|m| format!("{} ({})", m.price_label(), m.context_label()))
        .unwrap_or_else(|| "-".to_string());

    let models_block = Block::bordered()
        .title(Span::styled(
            theme::field_title(
                &format!("🤖 AI Modely ({} v katalogu OpenRouter)", state.models.len()),
                is_models_focused,
            ),
            Style::default()
                .fg(if is_models_focused { Color::Yellow } else { Color::White })
                .bold(),
        ))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::field_bg(is_models_focused)))
        .border_style(Style::default().fg(if is_models_focused { Color::Yellow } else { Color::DarkGray }));

    let models_lines = vec![
        Line::from(vec![
            Span::styled(
                if is_chat_sel { "  ▶ " } else { "    " },
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::styled("Parse / Chat model: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("◄ {} ►", state.chat_model),
                Style::default()
                    .fg(if is_chat_sel { Color::Rgb(45, 213, 183) } else { Color::White })
                    .add_modifier(if is_chat_sel { Modifier::BOLD } else { Modifier::empty() }),
            ),
        ]),
        Line::from(vec![
            Span::raw("      "),
            Span::styled("Cena & Kontext: ", Style::default().fg(Color::DarkGray)),
            Span::styled(chat_price, Style::default().fg(Color::Yellow)),
            Span::styled("  [Enter: vyhledat v katalogu]", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(
                if is_embed_sel { "  ▶ " } else { "    " },
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::styled("Embedding model:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("◄ {} ►", state.embedding_model),
                Style::default()
                    .fg(if is_embed_sel { Color::Rgb(255, 94, 219) } else { Color::White })
                    .add_modifier(if is_embed_sel { Modifier::BOLD } else { Modifier::empty() }),
            ),
        ]),
        Line::from(vec![
            Span::raw("      "),
            Span::styled("Cena & Kontext: ", Style::default().fg(Color::DarkGray)),
            Span::styled(embed_price, Style::default().fg(Color::Yellow)),
            Span::styled("  [Enter: vyhledat v katalogu]", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    let models_para = Paragraph::new(models_lines).block(models_block);
    frame.render_widget(models_para, area);
}
