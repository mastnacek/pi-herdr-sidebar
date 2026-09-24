//! Zen face — serene model/context/git/skill/MCP/quota overview.
use crate::slices::telemetry::{fmt_cost, fmt_tokens};
use crate::slices::view::state::SidebarState;
use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Paragraph, Wrap},
    Frame,
};

/// Serene, low-dopamine Zen view: complete model and context data, gentle tones.
pub fn render_zen_face(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let mut lines: Vec<Line> = Vec::new();

    let live = state.live.as_ref();
    let mcp = state.mcp.as_ref();
    let skill_state = state.skills.as_ref().and_then(|s| s.state.as_ref());

    lines.push(Line::raw(""));

    // 1. Model & Engine Identity (complete details)
    let model_str = live
        .map(|l| {
            if l.model_id.is_empty() {
                "neznámý model".to_string()
            } else {
                l.model_id.clone()
            }
        })
        .unwrap_or_else(|| "offline".to_string());

    let provider_str = live
        .map(|l| l.provider.clone())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "pi".to_string());

    let thinking_str = live
        .map(|l| l.thinking_level.clone())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "default".to_string());

    lines.push(Line::from(vec![
        Span::styled("Model: ", Style::default().fg(Color::DarkGray)),
        Span::styled(model_str, Style::default().fg(Color::White).bold()),
        Span::styled(
            format!("  ({})", provider_str),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Myšlení: ", Style::default().fg(Color::DarkGray)),
        Span::styled(thinking_str, Style::default().fg(Color::Gray)),
    ]));
    lines.push(Line::raw(""));

    // 2. Complete Context Window Telemetry
    let ctx_tokens = live.map(|l| l.context_tokens).unwrap_or(0);
    let ctx_window = live.map(|l| l.context_window).unwrap_or(0);
    let ctx_pct = live.and_then(|l| l.context_percent).unwrap_or(0.0);

    lines.push(Line::from(vec![
        Span::styled("Kontext: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(
                "{} / {} tok",
                fmt_tokens(ctx_tokens),
                fmt_tokens(ctx_window)
            ),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            format!("  ({:.1}%)", ctx_pct),
            Style::default().fg(Color::Gray),
        ),
    ]));

    // Subtle 12-cell bar
    let bar_len = 16usize;
    let filled = ((ctx_pct.min(100.0) / 100.0) * bar_len as f64).round() as usize;
    let pct_color = if ctx_pct >= 90.0 {
        Color::Red
    } else if ctx_pct >= 70.0 {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    lines.push(Line::from(vec![
        Span::raw("         "),
        Span::styled("█".repeat(filled), Style::default().fg(pct_color)),
        Span::styled(
            "░".repeat(bar_len.saturating_sub(filled)),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    // Detailed prompt token breakdown
    if let Some(l) = live {
        let prompt_total = l.input_tokens + l.cache_read + l.cache_write;
        lines.push(Line::from(vec![
            Span::styled("Tokeny:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("vstup: {} ", fmt_tokens(l.input_tokens)),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("výstup: {} ", fmt_tokens(l.output_tokens)),
                Style::default().fg(Color::Gray),
            ),
            if l.cache_read > 0 {
                Span::styled(
                    format!("keš: {} ", fmt_tokens(l.cache_read)),
                    Style::default().fg(Color::DarkGray),
                )
            } else {
                Span::raw("")
            },
            if l.reasoning_tokens > 0 {
                Span::styled(
                    format!("reasoning: {}", fmt_tokens(l.reasoning_tokens)),
                    Style::default().fg(Color::DarkGray),
                )
            } else {
                Span::raw("")
            },
        ]));

        lines.push(Line::from(vec![
            Span::styled("Náklady: ", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_cost(l.total_cost), Style::default().fg(Color::Gray)),
            if prompt_total > 0 && l.cache_read > 0 {
                let hit_ratio = (l.cache_read as f64 / prompt_total as f64) * 100.0;
                Span::styled(
                    format!("  (keš {:.0}%)", hit_ratio),
                    Style::default().fg(Color::DarkGray),
                )
            } else {
                Span::raw("")
            },
        ]));

        if let Some(or_credits) = &state.openrouter_credits {
            lines.extend(super::openrouter_ui::render_zen_openrouter_lines(
                or_credits,
            ));
        }
    }
    lines.push(Line::raw(""));

    // 3. Compact overview of other domains with subtle calm activity colors
    // Git
    if let Some(git) = live.and_then(|l| l.git.as_ref()) {
        let is_clean = git.staged == 0 && git.unstaged == 0 && git.untracked == 0;
        let mut git_spans = vec![
            Span::styled("Větev:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &git.branch,
                Style::default().fg(if is_clean { Color::Gray } else { Color::Cyan }),
            ),
        ];
        if is_clean {
            git_spans.push(Span::styled(
                " (čistý)",
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            git_spans.push(Span::styled(
                format!(" (+{} ~{} ?{})", git.staged, git.unstaged, git.untracked),
                Style::default().fg(Color::Yellow),
            ));
        }
        lines.push(Line::from(git_spans));
    }

    // Active skill: subtle green when loaded, gentle yellow when actively in-turn
    let has_skill = skill_state.and_then(|s| s.active_skill.as_deref());
    let skill_in_turn = skill_state.map(|s| s.in_turn).unwrap_or(false);
    let (skill_label, skill_color) = match (has_skill, skill_in_turn) {
        (Some(name), true) => (name, Color::Yellow),
        (Some(name), false) => (name, Color::Green),
        (None, _) => ("žádný", Color::DarkGray),
    };

    lines.push(Line::from(vec![
        Span::styled("Skill:   ", Style::default().fg(Color::DarkGray)),
        Span::styled(skill_label, Style::default().fg(skill_color)),
        if skill_in_turn {
            Span::styled(" (aktivní)", Style::default().fg(Color::Yellow))
        } else {
            Span::raw("")
        },
    ]));

    // MCP status: magenta/cyan when in-flight, gray when idle
    let mcp_count = mcp.map(|m| m.total_calls).unwrap_or(0);
    let mcp_tok = mcp.map(|m| m.total_tokens).unwrap_or(0);
    let mcp_in_flight = mcp.map(|m| m.in_flight).unwrap_or(false);

    let (mcp_text, mcp_color) = if mcp_in_flight {
        (
            format!(
                "{} volání (~{} tok) [přenáší…]",
                mcp_count,
                fmt_tokens(mcp_tok)
            ),
            Color::Magenta,
        )
    } else if mcp_count > 0 {
        (
            format!("{} volání (~{} tok)", mcp_count, fmt_tokens(mcp_tok)),
            Color::Gray,
        )
    } else {
        ("klid".to_string(), Color::DarkGray)
    };

    lines.push(Line::from(vec![
        Span::styled("MCP:     ", Style::default().fg(Color::DarkGray)),
        Span::styled(mcp_text, Style::default().fg(mcp_color)),
    ]));

    // 4. Sliding Quota Indicators (Antigravity upstream quota + Session sliding tokens)
    lines.push(Line::raw(""));

    // Exact parity with statusline: "🪐 Antigravity: 5h 34% (2h 28m) · Wk 48% (5d 21h)"
    if let Some(q) = &state.quota {
        if let Some(anti) = &q.antigravity {
            let col_cyan = Color::Rgb(95, 200, 230); // Soft cyan
            let col_lavender = Color::Rgb(170, 160, 220); // Lavender labels
            let col_dim = Color::Rgb(120, 124, 140); // Dim for timers and dots

            let get_capacity_color = |pct: u32| -> Color {
                if pct > 35 {
                    Color::Rgb(95, 200, 140) // Mint green (>35%)
                } else if pct > 15 {
                    Color::Rgb(230, 200, 90) // Warning amber/yellow (15%-35%)
                } else {
                    Color::Rgb(241, 108, 117) // Coral red (<15%)
                }
            };

            let mut anti_spans = vec![Span::styled(
                "🪐 Antigravity: ",
                Style::default().fg(col_cyan).bold(),
            )];

            let mut parts_added = false;

            if let Some(pct5) = anti.five_hour_pct {
                let col5 = get_capacity_color(pct5);
                anti_spans.push(Span::styled("5h ", Style::default().fg(col_lavender)));
                anti_spans.push(Span::styled(
                    format!("{}%", pct5),
                    Style::default().fg(col5).bold(),
                ));
                if let Some(t5) = &anti.five_hour_time {
                    anti_spans.push(Span::styled(
                        format!(" ({})", t5),
                        Style::default().fg(col_dim),
                    ));
                }
                parts_added = true;
            }

            if let Some(pct_wk) = anti.weekly_pct {
                if parts_added {
                    anti_spans.push(Span::styled(" · ", Style::default().fg(col_dim)));
                }
                let col_wk = get_capacity_color(pct_wk);
                anti_spans.push(Span::styled("Wk ", Style::default().fg(col_lavender)));
                anti_spans.push(Span::styled(
                    format!("{}%", pct_wk),
                    Style::default().fg(col_wk).bold(),
                ));
                if let Some(t_wk) = &anti.weekly_time {
                    anti_spans.push(Span::styled(
                        format!(" ({})", t_wk),
                        Style::default().fg(col_dim),
                    ));
                }
            }

            lines.push(Line::from(anti_spans));
        }

        // Secondary sliding window token metrics line
        let slide_spans = vec![
            Span::styled("Okno tok:", Style::default().fg(Color::DarkGray)),
            Span::raw(" 5m: "),
            Span::styled(
                fmt_tokens(q.session_sliding_5m_tokens),
                Style::default().fg(if q.session_sliding_5m_tokens > 100_000 {
                    Color::Yellow
                } else {
                    Color::Rgb(95, 200, 230)
                }),
            ),
            Span::styled(" │ 1h: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                fmt_tokens(q.session_sliding_1h_tokens),
                Style::default().fg(Color::Rgb(170, 160, 220)),
            ),
            Span::styled(" │ 5h: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                fmt_tokens(q.session_sliding_5h_tokens),
                Style::default().fg(Color::Gray),
            ),
        ];
        lines.push(Line::from(slide_spans));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Kvóty:   ", Style::default().fg(Color::DarkGray)),
            Span::styled("načítám stav…", Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines.extend(super::spai_ui::render_spai_lines(state));

    // 6. Weather (yr.no Locationforecast 2.0) — current + 10-day
    lines.push(Line::raw(""));
    lines.extend(super::weather_ui::render_weather_lines(state));

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Zen ");

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .scroll((state.scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}
