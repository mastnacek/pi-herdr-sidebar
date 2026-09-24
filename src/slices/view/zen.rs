//! Zen face — serene git, skill, MCP, quota, SPAI, and weather overview.
use crate::slices::telemetry::fmt_tokens;
use crate::slices::view::state::SidebarState;
use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Paragraph, Wrap},
    Frame,
};

/// Serene, low-dopamine Zen view: secondary domains (Git, Skill, MCP, Quota, SPAI, Weather).
pub fn render_zen_face(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let mut lines: Vec<Line> = Vec::new();

    let live = state.live.as_ref();
    let mcp = state.mcp.as_ref();
    let skill_state = state.skills.as_ref().and_then(|s| s.state.as_ref());

    lines.push(Line::raw(""));

    // 1. Compact overview of other domains with subtle calm activity colors
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

    // 2. Sliding Quota Indicators (Antigravity upstream quota + Session sliding tokens)
    lines.push(Line::raw(""));

    if let Some(q) = &state.quota {
        if let Some(anti) = &q.antigravity {
            let col_cyan = Color::Rgb(95, 200, 230);
            let col_lavender = Color::Rgb(170, 160, 220);
            let col_dim = Color::Rgb(120, 124, 140);

            let get_capacity_color = |pct: u32| -> Color {
                if pct > 35 {
                    Color::Rgb(95, 200, 140)
                } else if pct > 15 {
                    Color::Rgb(230, 200, 90)
                } else {
                    Color::Rgb(241, 108, 117)
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

    // 3. Weather (yr.no Locationforecast 2.0)
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
