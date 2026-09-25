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
        Color::Rgb(95, 200, 140)
    } else if remaining > 2.0 {
        Color::Rgb(230, 200, 90)
    } else {
        Color::Rgb(241, 108, 117)
    }
}

pub fn shared_banner_height(state: &SidebarState) -> u16 {
    if state.active_tab == crate::slices::view::state::Tab::Notes {
        return 1;
    }

    let mut base_lines = 4;
    if let Some(l) = &state.live {
        let prompt = l.input_tokens + l.cache_read + l.cache_write;
        if prompt > 0 && l.cache_read > 0 {
            base_lines += 1;
        }
    }
    if let Some(q) = &state.quota {
        if q.antigravity.is_some() {
            base_lines += 1;
        }
    }
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
    (base_lines + or_lines + 2) as u16
}

fn build_model_and_turns_lines(state: &SidebarState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let live = state.live.as_ref();
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
        Span::styled(
            format!("({}) ", provider),
            Style::default().fg(Color::DarkGray),
        ),
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

    if let Some(l) = live {
        lines.push(Line::from(vec![
            Span::styled("  ⚡ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{} tahů", l.turns_count),
                Style::default().fg(Color::White),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} nástrojů", l.tool_calls_count),
                Style::default().fg(Color::Cyan),
            ),
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
    lines
}

fn build_context_and_cost_lines(state: &SidebarState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let live = state.live.as_ref();
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
        Span::styled(
            format!(" {:.1}%", ctx_pct),
            Style::default().fg(pct_color).bold(),
        ),
        Span::styled(
            format!(" ({}/{})", fmt_tokens(ctx_tokens), fmt_tokens(ctx_window)),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    if let Some(l) = live {
        lines.push(Line::from(vec![
            Span::styled(
                format!("💰 {}", fmt_cost(l.total_cost)),
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("⬆️ {}", fmt_tokens(l.input_tokens)),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("⬇️ {}", fmt_tokens(l.output_tokens)),
                Style::default().fg(Color::Green),
            ),
            if l.reasoning_tokens > 0 {
                Span::styled(
                    format!(" │ 🧠 {}", fmt_tokens(l.reasoning_tokens)),
                    Style::default().fg(Color::Magenta),
                )
            } else {
                Span::raw("")
            },
        ]));

        let prompt = l.input_tokens + l.cache_read + l.cache_write;
        if prompt > 0 && l.cache_read > 0 {
            let hit_pct = (l.cache_read as f64 / prompt as f64) * 100.0;
            lines.push(Line::from(vec![
                Span::styled("📦 Mezipaměť: ", Style::default().fg(Color::Cyan).bold()),
                Span::styled(
                    format!("čtení: {} ", fmt_tokens(l.cache_read)),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(
                    format!("zápis: {} ", fmt_tokens(l.cache_write)),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("(🎯 úspora {:.0}%)", hit_pct),
                    Style::default().fg(Color::Green).bold(),
                ),
            ]));
        }
    } else {
        lines.push(Line::from(vec![Span::styled(
            "💰 $0.00",
            Style::default().fg(Color::DarkGray),
        )]));
    }
    lines
}

fn build_quota_and_credits_lines(state: &SidebarState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let Some(anti) = state.quota.as_ref().and_then(|q| q.antigravity.as_ref()) {
        let col_lavender = Color::Rgb(170, 160, 220);
        let col_dim = Color::Rgb(120, 124, 140);
        let cap_col = |p: u32| {
            if p > 35 {
                Color::Rgb(95, 200, 140)
            } else if p > 15 {
                Color::Rgb(230, 200, 90)
            } else {
                Color::Rgb(241, 108, 117)
            }
        };

        let mut spans = vec![Span::styled(
            "🪐 Antigravity: ",
            Style::default().fg(Color::Rgb(95, 200, 230)).bold(),
        )];
        let mut added = false;
        if let Some(pct5) = anti.five_hour_pct {
            spans.push(Span::styled("5h ", Style::default().fg(col_lavender)));
            spans.push(Span::styled(
                format!("{}%", pct5),
                Style::default().fg(cap_col(pct5)).bold(),
            ));
            if let Some(t5) = &anti.five_hour_time {
                spans.push(Span::styled(
                    format!(" ({})", t5),
                    Style::default().fg(col_dim),
                ));
            }
            added = true;
        }
        if let Some(pct_wk) = anti.weekly_pct {
            if added {
                spans.push(Span::styled(" · ", Style::default().fg(col_dim)));
            }
            spans.push(Span::styled("Wk ", Style::default().fg(col_lavender)));
            spans.push(Span::styled(
                format!("{}%", pct_wk),
                Style::default().fg(cap_col(pct_wk)).bold(),
            ));
            if let Some(tw) = &anti.weekly_time {
                spans.push(Span::styled(
                    format!(" ({})", tw),
                    Style::default().fg(col_dim),
                ));
            }
        }
        lines.push(Line::from(spans));
    }

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
                    format!(
                        " (vyčerpáno ${:.2} / ${:.2})",
                        acc.total_usage, acc.total_credits
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        } else if or.accounts.len() > 1 {
            lines.push(Line::from(vec![Span::styled(
                "💳 Kredity OpenRouter:",
                Style::default().fg(Color::Yellow).bold(),
            )]));
            for acc in &or.accounts {
                let col = credit_color(acc.remaining_credits);
                lines.push(Line::from(vec![
                    Span::styled("   ● ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{}: ", acc.label),
                        Style::default().fg(Color::White).bold(),
                    ),
                    Span::styled(
                        format!("${:.2} zbývá ", acc.remaining_credits),
                        Style::default().fg(col).bold(),
                    ),
                    Span::styled(
                        format!(
                            "(vyčerpáno ${:.2} / ${:.2})",
                            acc.total_usage, acc.total_credits
                        ),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }
    }
    lines
}

pub fn render_shared_model_banner(frame: &mut Frame, area: Rect, state: &SidebarState) {
    if state.active_tab == crate::slices::view::state::Tab::Notes {
        render_minimalist_notes_banner(frame, area, state);
        return;
    }

    let mut lines = Vec::new();
    lines.extend(build_model_and_turns_lines(state));
    lines.extend(build_context_and_cost_lines(state));
    lines.extend(build_quota_and_credits_lines(state));

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Model & Kredity ");

    let paragraph = Paragraph::new(Text::from(lines)).block(block);
    frame.render_widget(paragraph, area);
}

fn render_minimalist_notes_banner(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let live = state.live.as_ref();
    let model = live
        .map(|l| {
            if l.model_id.is_empty() {
                "neznámý model".to_string()
            } else {
                l.model_id.clone()
            }
        })
        .unwrap_or_else(|| "offline".to_string());

    let provider = live
        .map(|l| l.provider.as_str())
        .filter(|p| !p.is_empty())
        .unwrap_or("pi");

    let ctx_pct = live.and_then(|l| l.context_percent).unwrap_or(0.0);
    let ctx_tokens = live.map(|l| l.context_tokens).unwrap_or(0);
    let ctx_window = live.map(|l| l.context_window).unwrap_or(0);

    let pct_color = if ctx_pct >= 90.0 {
        Color::Red
    } else if ctx_pct >= 60.0 {
        Color::Yellow
    } else {
        Color::Green
    };

    let line = Line::from(vec![
        Span::styled(" 🤖 ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}/", provider), Style::default().fg(Color::DarkGray)),
        Span::styled(model, Style::default().fg(Color::Cyan).bold()),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled("📊 ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:.1}%", ctx_pct),
            Style::default().fg(pct_color).bold(),
        ),
        Span::styled(
            format!(" ({}/{})", fmt_tokens(ctx_tokens), fmt_tokens(ctx_window)),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}
