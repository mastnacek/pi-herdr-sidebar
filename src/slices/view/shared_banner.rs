use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

use super::ui::spinner_char;
use crate::slices::telemetry::{fmt_cost, fmt_tokens};
use crate::slices::view::state::SidebarState;

fn credit_color(remaining: f64) -> Color {
    if remaining > 10.0 {
        Color::Rgb(95, 200, 140) // Mint green (> $10)
    } else if remaining > 2.0 {
        Color::Rgb(230, 200, 90) // Amber / yellow ($2 - $10)
    } else {
        Color::Rgb(241, 108, 117) // Coral red (< $2)
    }
}

/// Computes the required height for the shared model banner.
pub fn shared_banner_height(state: &SidebarState) -> u16 {
    let base_lines = 4; // Model identity, Turn/Activity, Context bar, Token/Cost
    let or_lines = if let Some(or) = &state.openrouter_credits {
        if or.accounts.is_empty() {
            0
        } else if or.accounts.len() == 1 {
            1
        } else {
            1 + or.accounts.len()
        }
    } else {
        0
    };
    // 2 for borders (top + bottom) + content lines
    (base_lines + or_lines + 2) as u16
}

/// Renders the shared model/cost/context/credits banner in the status tab's vivid color style.
pub fn render_shared_model_banner(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let mut lines: Vec<Line> = Vec::new();
    let live = state.live.as_ref();

    // 1. Model & Engine Identity line
    let spinner = spinner_char(state.anim_tick);
    let is_working = live.map(|l| l.is_working).unwrap_or(false);
    let status_icon = if is_working {
        Span::styled(
            format!("{} ", spinner),
            Style::default().fg(Color::Cyan).bold(),
        )
    } else {
        Span::styled("● ", Style::default().fg(Color::Green))
    };

    let provider = live
        .map(|l| l.provider.as_str())
        .filter(|p| !p.is_empty())
        .unwrap_or("pi");

    let model_id = live
        .map(|l| {
            if l.model_id.is_empty() {
                "neznámý model".to_string()
            } else {
                l.model_id.clone()
            }
        })
        .unwrap_or_else(|| "offline".to_string());

    let mut model_line = vec![
        status_icon,
        Span::styled(format!("({}) ", provider), Style::default().fg(Color::DarkGray)),
        Span::styled(model_id, Style::default().fg(Color::Green).bold()),
    ];

    if let Some(l) = live {
        if !l.thinking_level.is_empty() {
            model_line.push(Span::styled(" • 🧠 ", Style::default().fg(Color::DarkGray)));
            model_line.push(Span::styled(
                l.thinking_level.clone(),
                Style::default().fg(Color::Cyan),
            ));
        }
    }
    lines.push(Line::from(model_line));

    // 2. Turns & Tool Activity summary
    if let Some(l) = live {
        lines.push(Line::from(vec![
            Span::styled("  ⚡ ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("{} tahů", l.turns_count), Style::default().fg(Color::White)),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} nástrojů", l.tool_calls_count), Style::default().fg(Color::Cyan)),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            if l.tool_errors_count > 0 {
                Span::styled(
                    format!("⚠️ {} chyb", l.tool_errors_count),
                    Style::default().fg(Color::Red).bold(),
                )
            } else {
                Span::styled("✓ 0 chyb", Style::default().fg(Color::Green))
            },
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  ⚡ ", Style::default().fg(Color::DarkGray)),
            Span::styled("čekám na aktivitu…", Style::default().fg(Color::DarkGray)),
        ]));
    }

    // 3. Context Window Usage bar
    let ctx_tokens = live.map(|l| l.context_tokens).unwrap_or(0);
    let ctx_window = live.map(|l| l.context_window).unwrap_or(0);
    let ctx_pct = live.and_then(|l| l.context_percent).unwrap_or(0.0);

    let bar_w = 16usize;
    let filled = ((ctx_pct.min(100.0) / 100.0) * bar_w as f64).round() as usize;
    let pct_color = if ctx_pct >= 90.0 {
        Color::Red
    } else if ctx_pct >= 60.0 {
        Color::Yellow
    } else {
        Color::Green
    };

    lines.push(Line::from(vec![
        Span::styled("📊 Kontext: ", Style::default().fg(Color::Cyan).bold()),
        Span::styled(
            format!(
                "{}{}",
                "█".repeat(filled),
                "░".repeat(bar_w.saturating_sub(filled))
            ),
            Style::default().fg(pct_color),
        ),
        Span::styled(format!(" {:.1}%", ctx_pct), Style::default().fg(pct_color).bold()),
        Span::styled(
            format!(" ({}/{})", fmt_tokens(ctx_tokens), fmt_tokens(ctx_window)),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    // 4. Token & Cost Telemetry
    if let Some(l) = live {
        lines.push(Line::from(vec![
            Span::styled(
                format!("💰 {}", fmt_cost(l.total_cost)),
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("⬆️ {}", fmt_tokens(l.input_tokens)), Style::default().fg(Color::Cyan)),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("⬇️ {}", fmt_tokens(l.output_tokens)), Style::default().fg(Color::Green)),
            if l.cache_read > 0 {
                Span::styled(
                    format!(" │ 📦 {}", fmt_tokens(l.cache_read)),
                    Style::default().fg(Color::Gray),
                )
            } else {
                Span::raw("")
            },
            if l.reasoning_tokens > 0 {
                Span::styled(
                    format!(" │ 🧠 {}", fmt_tokens(l.reasoning_tokens)),
                    Style::default().fg(Color::Magenta),
                )
            } else {
                Span::raw("")
            },
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("💰 $0.00", Style::default().fg(Color::DarkGray)),
        ]));
    }

    // 5. OpenRouter credits (if any OpenRouter accounts configured)
    if let Some(or) = &state.openrouter_credits {
        if or.accounts.len() == 1 {
            let acc = &or.accounts[0];
            let col = credit_color(acc.remaining_credits);
            lines.push(Line::from(vec![
                Span::styled("💳 Kredity: ", Style::default().fg(Color::Yellow).bold()),
                Span::styled(
                    format!("${:.2} zbývá", acc.remaining_credits),
                    Style::default().fg(col).bold(),
                ),
                Span::styled(
                    format!(" (vyčerpáno ${:.2} / ${:.2})", acc.total_usage, acc.total_credits),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        } else if or.accounts.len() > 1 {
            lines.push(Line::from(vec![
                Span::styled("💳 Kredity OpenRouter:", Style::default().fg(Color::Yellow).bold()),
            ]));
            for acc in &or.accounts {
                let col = credit_color(acc.remaining_credits);
                lines.push(Line::from(vec![
                    Span::styled("   ● ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{}: ", acc.label), Style::default().fg(Color::White).bold()),
                    Span::styled(
                        format!("${:.2} zbývá ", acc.remaining_credits),
                        Style::default().fg(col).bold(),
                    ),
                    Span::styled(
                        format!("(vyčerpáno ${:.2} / ${:.2})", acc.total_usage, acc.total_credits),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }
    }

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Model & Kredity ");

    let paragraph = Paragraph::new(Text::from(lines)).block(block);
    frame.render_widget(paragraph, area);
}
