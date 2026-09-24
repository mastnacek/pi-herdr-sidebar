use crate::slices::telemetry::{fmt_cost, fmt_tokens};
use crate::slices::view::state::{SidebarState, Tab};
use ansi_to_tui::IntoText;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span, Text},
    widgets::{Block, BorderType, LineGauge, Paragraph, Tabs, Wrap},
    Frame,
};

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn spinner_char(tick: u64) -> &'static str {
    SPINNER_FRAMES[(tick as usize) % SPINNER_FRAMES.len()]
}

pub fn render(frame: &mut Frame, state: &SidebarState) {
    let area = frame.area();

    if state.refresh_timer > 0 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
            .split(area);

        render_header(frame, chunks[0], state);

        let gauge = LineGauge::default()
            .filled_style(Style::default().fg(Color::Yellow).bold())
            .unfilled_style(Style::default().fg(Color::DarkGray))
            .line_set(symbols::line::THICK)
            .ratio(state.refresh_progress.clamp(0.0, 1.0))
            .label(Span::styled(
                format!(" ⟳ REFRESHING [{}] ", state.refresh_status),
                Style::default().fg(Color::Yellow).bold(),
            ));
        frame.render_widget(gauge, chunks[1]);
        render_body(frame, chunks[2], state);
        render_footer(frame, chunks[3], state);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
            .split(area);

        render_header(frame, chunks[0], state);
        render_body(frame, chunks[1], state);
        render_footer(frame, chunks[2], state);
    }

    if state.weather_popup {
        render_weather_popup(frame, area, state);
    }
}

/// Centered location selector popup (↑/↓/j/k, Enter, Esc).
fn render_weather_popup(frame: &mut Frame, area: Rect, state: &SidebarState) {
    use ratatui::widgets::Clear;

    let width = 32u16.min(area.width.saturating_sub(2));
    let height = (crate::slices::telemetry::weather_live::LOCATIONS.len() as u16 + 2)
        .min(area.height.saturating_sub(2));

    let popup = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    frame.render_widget(Clear, popup);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Vybrat lokalitu ");

    let rows: Vec<Line> = crate::slices::telemetry::weather_live::LOCATIONS
        .iter()
        .enumerate()
        .map(|(i, loc)| {
            let is_cursor = i == state.weather_popup_cursor;
            let is_selected = i == state.weather_location_index;
            let marker = if is_cursor { "▶" } else { " " };
            let check = if is_selected { " ●" } else { "" };
            let style = if is_cursor {
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Rgb(30, 35, 55))
                    .bold()
            } else if is_selected {
                Style::default().fg(Color::Cyan).bold()
            } else {
                Style::default().fg(Color::Gray)
            };
            Line::from(Span::styled(
                format!("{} {}{}", marker, loc.name, check),
                style,
            ))
        })
        .collect();

    frame.render_widget(Paragraph::new(rows).block(block), popup);
}

fn render_header(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let selected_index = state.active_tab.to_index();
    let spinner = spinner_char(state.anim_tick);

    let is_zen = state.active_tab == Tab::Zen;

    // 0. Zen tab: calm, serene indicator
    let zen_spans = vec![Span::raw(" 0: Zen ")];

    // 1. Status tab indicator: spinner ONLY when agent is actively working/executing (and not in Zen tab)
    let is_agent_working = state.live.as_ref().map(|l| l.is_working).unwrap_or(false);
    let status_spans = if is_agent_working && !is_zen {
        vec![
            Span::raw(" 1: Status "),
            Span::styled(spinner, Style::default().fg(Color::Cyan).bold()),
            Span::raw(" "),
        ]
    } else {
        vec![Span::raw(" 1: Status ")]
    };

    // 2. Skills tab indicator: spinner ONLY when skill is active AND in-turn (and not in Zen tab)
    let is_skill_working = state
        .skills
        .as_ref()
        .and_then(|f| f.state.as_ref())
        .map(|s| s.in_turn && s.active_skill.is_some())
        .unwrap_or(false);

    let skills_spans = if is_skill_working && !is_zen {
        vec![
            Span::raw(" 2: Skills "),
            Span::styled(spinner, Style::default().fg(Color::Yellow).bold()),
            Span::raw(" "),
        ]
    } else {
        vec![Span::raw(" 2: Skills ")]
    };

    // 3. MCP tab indicator: spinner when MCP calls are in flight (and not in Zen tab)
    let is_mcp_in_flight = state.mcp.as_ref().map(|m| m.in_flight).unwrap_or(false);
    let mcp_spans = if is_mcp_in_flight && !is_zen {
        vec![
            Span::raw(" 3: MCP "),
            Span::styled(spinner, Style::default().fg(Color::Magenta).bold()),
            Span::raw(" "),
        ]
    } else {
        vec![Span::raw(" 3: MCP ")]
    };

    let titles: Vec<Line> = vec![
        Line::from(zen_spans),
        Line::from(status_spans),
        Line::from(skills_spans),
        Line::from(mcp_spans),
    ];

    // Top status indicator: static dot in Zen tab or when idle; animated ONLY when agent is running in active tabs
    let live_indicator = if is_agent_working && !is_zen {
        Span::styled(
            format!(" {} BĚŽÍ ", spinner),
            Style::default().fg(Color::Yellow).bold(),
        )
    } else if is_agent_working && is_zen {
        Span::styled(" ● BĚŽÍ ", Style::default().fg(Color::Yellow))
    } else if state.live.is_some() || state.snapshot.as_ref().is_some_and(|s| s.live) {
        Span::styled(" ● PŘIPRAVEN ", Style::default().fg(Color::Green).bold())
    } else if state.snapshot.is_some() {
        Span::styled(" ⟳ OBNOVA ", Style::default().fg(Color::Yellow).bold())
    } else {
        Span::styled(" ○ NEČINNÝ ", Style::default().fg(Color::DarkGray))
    };

    let title_line = Line::from(vec![
        Span::styled(
            " ⚡ Herdr Pi-Sidebar ",
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::styled(
            format!("[{}] ", state.refresh_status),
            Style::default().fg(Color::DarkGray),
        ),
        live_indicator,
    ]);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(title_line);

    let tabs = Tabs::new(titles)
        .block(block)
        .select(selected_index)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::Rgb(30, 35, 55))
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");

    frame.render_widget(tabs, area);
}

fn render_body(frame: &mut Frame, area: Rect, state: &SidebarState) {
    match state.active_tab {
        Tab::Zen => render_zen_face(frame, area, state),
        Tab::Mcp => render_mcp_face(frame, area, state),
        Tab::Status => {
            if let Some(t) = &state.live {
                render_live_status(frame, area, t, state);
            } else if let Some(snapshot) = &state.snapshot {
                render_status_face(frame, area, snapshot, state.scroll);
            } else {
                render_empty_state(frame, area, state);
            }
        }
        Tab::Skills => {
            if let Some(skills) = &state.skills {
                render_skills_live(frame, area, skills, state);
            } else if let Some(snapshot) = &state.snapshot {
                render_skills_face(frame, area, snapshot, state.scroll);
            } else {
                render_empty_state(frame, area, state);
            }
        }
    }
}

/// Native statusline-parity face rendered straight from LiveTelemetry.
fn render_live_status(
    frame: &mut Frame,
    area: Rect,
    t: &crate::slices::telemetry::LiveTelemetry,
    state: &SidebarState,
) {
    let mut lines: Vec<Line> = Vec::new();

    // 1. Model & Engine Identity
    let spinner = spinner_char(state.anim_tick);
    let status_icon = if t.is_working {
        Span::styled(
            format!("{} ", spinner),
            Style::default().fg(Color::Cyan).bold(),
        )
    } else {
        Span::styled("● ", Style::default().fg(Color::Green))
    };

    let mut model_line = vec![
        status_icon,
        Span::styled(
            format!("({}) ", t.provider),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            if t.model_id.is_empty() {
                "neznámý model".to_string()
            } else {
                t.model_id.clone()
            },
            Style::default().fg(Color::Green).bold(),
        ),
    ];
    if !t.thinking_level.is_empty() {
        model_line.push(Span::styled(" • 🧠 ", Style::default().fg(Color::DarkGray)));
        model_line.push(Span::styled(
            t.thinking_level.clone(),
            Style::default().fg(Color::Cyan),
        ));
    }
    lines.push(Line::from(model_line));

    // Session Turn & Tool Activity summary
    lines.push(Line::from(vec![
        Span::styled("  ⚡ ", Style::default().fg(Color::Yellow)),
        Span::styled(
            format!("{} tahů", t.turns_count),
            Style::default().fg(Color::White),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} nástrojů", t.tool_calls_count),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        if t.tool_errors_count > 0 {
            Span::styled(
                format!("⚠️ {} chyb", t.tool_errors_count),
                Style::default().fg(Color::Red).bold(),
            )
        } else {
            Span::styled("✓ 0 chyb", Style::default().fg(Color::Green))
        },
    ]));
    lines.push(Line::raw(""));

    // 2. Git Status Section
    if let Some(git) = &t.git {
        lines.push(Line::from(vec![
            Span::styled("🌿 Git: ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(&git.branch, Style::default().fg(Color::White).bold()),
            Span::raw(" "),
            if git.ahead > 0 || git.behind > 0 {
                Span::styled(
                    format!("[⇡{} ⇣{}]", git.ahead, git.behind),
                    Style::default().fg(Color::Yellow).bold(),
                )
            } else {
                Span::styled("[synced]", Style::default().fg(Color::Green))
            },
        ]));

        // Working tree details: staged, unstaged, untracked
        let is_clean = git.staged == 0 && git.unstaged == 0 && git.untracked == 0;
        let mut status_spans = vec![Span::raw("   ")];
        if is_clean {
            status_spans.push(Span::styled(
                "✓ pracovní strom čistý",
                Style::default().fg(Color::Green),
            ));
        } else {
            if git.staged > 0 {
                status_spans.push(Span::styled(
                    format!("● připraveno: {} ", git.staged),
                    Style::default().fg(Color::Green),
                ));
            }
            if git.unstaged > 0 {
                status_spans.push(Span::styled(
                    format!("● změněno: {} ", git.unstaged),
                    Style::default().fg(Color::Yellow),
                ));
            }
            if git.untracked > 0 {
                status_spans.push(Span::styled(
                    format!("? nesledováno: {}", git.untracked),
                    Style::default().fg(Color::Red),
                ));
            }
        }
        lines.push(Line::from(status_spans));

        // Latest commit info
        if !git.commit_hash.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("   commit: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&git.commit_hash, Style::default().fg(Color::Magenta).bold()),
                Span::styled(
                    format!(" ({}) ", git.commit_age),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    if git.commit_msg.len() > 32 {
                        format!("{}…", &git.commit_msg[..32])
                    } else {
                        git.commit_msg.clone()
                    },
                    Style::default().fg(Color::Gray),
                ),
            ]));
        }
    } else if !t.git_branch.is_empty() {
        let state_icon = if t.git_dirty > 0 {
            Span::styled(
                format!(" ●{}", t.git_dirty),
                Style::default().fg(Color::Red),
            )
        } else {
            Span::styled(" ○čistý", Style::default().fg(Color::Green))
        };
        lines.push(Line::from(vec![
            Span::styled("🌿 ", Style::default()),
            Span::styled(
                t.git_branch.clone(),
                Style::default().fg(Color::Cyan).bold(),
            ),
            state_icon,
        ]));
    }
    lines.push(Line::raw(""));

    // 3. Context Window Usage
    if let Some(pct) = t.context_percent {
        let bar_w = 16usize;
        let filled = ((pct.min(100.0) / 100.0) * bar_w as f64).round() as usize;
        let pct_color = if pct >= 90.0 {
            Color::Red
        } else if pct >= 60.0 {
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
                format!(" {:.1}%", pct),
                Style::default().fg(pct_color).bold(),
            ),
            Span::styled(
                format!(
                    " ({}/{})",
                    fmt_tokens(t.context_tokens),
                    fmt_tokens(t.context_window)
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    // 4. Token & Cost Telemetry
    lines.push(Line::from(vec![
        Span::styled(
            format!("💰 {}", fmt_cost(t.total_cost)),
            Style::default().fg(Color::Yellow).bold(),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("⬆️  {}", fmt_tokens(t.input_tokens)),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("⬇️ {}", fmt_tokens(t.output_tokens)),
            Style::default().fg(Color::Green),
        ),
        if t.reasoning_tokens > 0 {
            Span::styled(
                format!(" │ 🧠 {}", fmt_tokens(t.reasoning_tokens)),
                Style::default().fg(Color::Magenta),
            )
        } else {
            Span::raw("")
        },
    ]));

    // Cache hits & ratio
    let prompt = t.input_tokens + t.cache_read + t.cache_write;
    if prompt > 0 {
        let hit_pct = (t.cache_read as f64 / prompt as f64) * 100.0;
        lines.push(Line::from(vec![
            Span::styled("📦 Mezipaměť: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("čtení: {} ", fmt_tokens(t.cache_read)),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("zápis: {} ", fmt_tokens(t.cache_write)),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("(🎯 úspora {:.0}%)", hit_pct),
                Style::default().fg(Color::Green).bold(),
            ),
        ]));
    }

    // Footer diagnostics: session id + timestamp
    lines.push(Line::raw(""));
    let mut footer_spans = vec![
        Span::styled("⚡ relace: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            t.session_id.chars().take(8).collect::<String>(),
            Style::default().fg(Color::Magenta).bold(),
        ),
    ];
    if !t.last_entry_ts.is_empty() {
        let time_part = t
            .last_entry_ts
            .split('T')
            .nth(1)
            .and_then(|s| s.split('.').next())
            .unwrap_or("");
        if !time_part.is_empty() {
            footer_spans.push(Span::styled(
                format!(" @ {}", time_part),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }
    lines.push(Line::from(footer_spans));

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Telemetrie & Stav ");

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .scroll((state.scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

/// Serene, low-dopamine Zen view: complete model and context data, gentle tones.
fn render_zen_face(frame: &mut Frame, area: Rect, state: &SidebarState) {
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

    // 5. SPAI Tasks Ledger (with Linkarzu TrueColor Palette from mozek_rust)
    lines.push(Line::raw(""));
    let spai_title_spans = vec![Span::styled(
        "SPAI:    ",
        Style::default().fg(Color::DarkGray),
    )];

    if let Some(spai) = &state.spai {
        let counts = &spai.counts;
        let mut ribbon_spans = spai_title_spans;

        // Render multi-segment ribbon: done (mint) | working (yellow) | waiting (violet) | todo (pink) | cancelled (slate)
        let ribbon_len = 16usize;
        let total = counts.total_tasks;

        if total > 0 {
            let seg = |count: usize| -> usize {
                ((count as f64 / total as f64) * ribbon_len as f64).round() as usize
            };

            let s_done = seg(counts.done);
            let s_work = seg(counts.working);
            let s_wait = seg(counts.waiting);
            let s_cancel = seg(counts.cancelled);
            let s_todo = ribbon_len.saturating_sub(s_done + s_work + s_wait + s_cancel);

            // Truecolors from SPAI Linkarzu Palette
            let col_done = Color::Rgb(55, 244, 153); // #37f499 neon mint
            let col_work = Color::Rgb(241, 252, 121); // #f1fc79 electric yellow
            let col_wait = Color::Rgb(152, 122, 251); // #987afb neon violet
            let col_todo = Color::Rgb(249, 77, 255); // #f94dff vivid pink
            let col_cancel = Color::Rgb(135, 145, 170); // #8791aa slate

            if s_done > 0 {
                ribbon_spans.push(Span::styled(
                    "█".repeat(s_done),
                    Style::default().fg(col_done),
                ));
            }
            if s_work > 0 {
                ribbon_spans.push(Span::styled(
                    "█".repeat(s_work),
                    Style::default().fg(col_work),
                ));
            }
            if s_wait > 0 {
                ribbon_spans.push(Span::styled(
                    "█".repeat(s_wait),
                    Style::default().fg(col_wait),
                ));
            }
            if s_todo > 0 {
                ribbon_spans.push(Span::styled(
                    "█".repeat(s_todo),
                    Style::default().fg(col_todo),
                ));
            }
            if s_cancel > 0 {
                ribbon_spans.push(Span::styled(
                    "░".repeat(s_cancel),
                    Style::default().fg(col_cancel),
                ));
            }

            let pct = (counts.done as f64 / total as f64 * 100.0).round() as usize;
            ribbon_spans.push(Span::styled(
                format!(" [{}/{}] {}%", counts.done, total, pct),
                Style::default().fg(col_done).bold(),
            ));
            lines.push(Line::from(ribbon_spans));

            // Breakdown counts
            let mut stat_spans = vec![Span::raw("         ")];
            if counts.working > 0 {
                stat_spans.push(Span::styled(
                    format!("◐ {} běží  ", counts.working),
                    Style::default().fg(col_work),
                ));
            }
            if counts.waiting > 0 {
                stat_spans.push(Span::styled(
                    format!("⏳ {} čeká  ", counts.waiting),
                    Style::default().fg(col_wait),
                ));
            }
            if counts.todo > 0 {
                stat_spans.push(Span::styled(
                    format!("○ {} úkolů  ", counts.todo),
                    Style::default().fg(col_todo),
                ));
            }
            if counts.ideas > 0 {
                stat_spans.push(Span::styled(
                    format!("💡 {} nápadů", counts.ideas),
                    Style::default().fg(Color::Rgb(4, 209, 249)), // Cyan
                ));
            }
            lines.push(Line::from(stat_spans));

            // List 2-3 most recent or working tasks
            let mut shown = 0;
            // Prioritize working, waiting, and open todo
            for r in spai.index.records.iter().rev() {
                let st = r.status.to_lowercase();
                if st == "working" || st == "waiting" || st == "todo" {
                    let (icon, color) = match st.as_str() {
                        "working" => ("◐", col_work),
                        "waiting" => ("⏳", col_wait),
                        _ => ("○", col_todo),
                    };

                    let title = if r.title.len() > 36 {
                        format!("{}…", &r.title[..36])
                    } else {
                        r.title.clone()
                    };

                    lines.push(Line::from(vec![
                        Span::raw("         "),
                        Span::styled(format!("{} ", icon), Style::default().fg(color).bold()),
                        Span::styled(format!("{}: ", r.id), Style::default().fg(Color::DarkGray)),
                        Span::styled(title, Style::default().fg(Color::Gray)),
                    ]));
                    shown += 1;
                    if shown >= 3 {
                        break;
                    }
                }
            }
        } else {
            ribbon_spans.push(Span::styled(
                "žádné úkoly v docs/spai",
                Style::default().fg(Color::DarkGray),
            ));
            lines.push(Line::from(ribbon_spans));
        }
    } else {
        let mut no_spai_spans = spai_title_spans;
        no_spai_spans.push(Span::styled(
            "bez docs/spai ledgeru",
            Style::default().fg(Color::DarkGray),
        ));
        lines.push(Line::from(no_spai_spans));
    }

    // 6. Weather (yr.no Locationforecast 2.0) — current + 7-day
    lines.push(Line::raw(""));
    lines.extend(render_weather_lines(state));

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

/// Weather face lines: current conditions + 7-day forecast row list.
/// Source: MET Norway (yr.no) Locationforecast 2.0 — no API key, User-Agent identified.
fn render_weather_lines(state: &SidebarState) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let Some(w) = &state.weather else {
        lines.push(Line::from(vec![
            Span::styled("Počasí:   ", Style::default().fg(Color::DarkGray)),
            Span::styled("načítám…", Style::default().fg(Color::DarkGray)),
        ]));
        return lines;
    };

    if let Some(err) = &w.error {
        lines.push(Line::from(vec![
            Span::styled("Počasí:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(err.clone(), Style::default().fg(Color::Red)),
        ]));
        return lines;
    }

    // Header: location + selected marker + key hint
    let loc_name = w.location_name.clone();
    let idx = w.location_index;
    let total = crate::slices::telemetry::weather_live::LOCATIONS.len();
    lines.push(Line::from(vec![
        Span::styled("Počasí:   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("[{} / {}] ", idx + 1, total),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(loc_name, Style::default().fg(Color::Cyan).bold()),
        Span::styled("  (w=vybrat)", Style::default().fg(Color::DarkGray)),
    ]));

    // Current conditions: big icon + temp + wind + humidity
    if let Some(cur) = &w.current {
        let (r, g, b) = cur.color;
        let mut cur_line = vec![
            Span::raw("         "),
            Span::styled(
                format!("{} ", cur.icon),
                Style::default().fg(Color::Rgb(r, g, b)).bold(),
            ),
            Span::styled(
                format!("{:.1}°C", cur.temp_c),
                Style::default().fg(Color::White).bold(),
            ),
        ];
        if cur.wind_ms > 0.0 {
            cur_line.push(Span::styled(
                format!("  💨 {:.1} m/s", cur.wind_ms),
                Style::default().fg(Color::Gray),
            ));
        }
        if let Some(h) = cur.humidity {
            cur_line.push(Span::styled(
                format!("  💧 {:.0}%", h),
                Style::default().fg(Color::Rgb(4, 209, 249)),
            ));
        }
        lines.push(Line::from(cur_line));
    }

    // 7-day forecast: today first
    if !w.days.is_empty() {
        lines.push(Line::from(Span::styled(
            "Týden:    ".to_string(),
            Style::default().fg(Color::DarkGray),
        )));
        for day in &w.days {
            let (r, g, b) = day.color;
            // "2026-09-25" → "25.9."
            let date_parts: Vec<&str> = day.date.split('-').collect();
            let date_label = if date_parts.len() == 3 {
                let m = date_parts[1].trim_start_matches('0');
                let d = date_parts[2].trim_start_matches('0');
                format!("{}.{}. ", d, m)
            } else {
                String::new()
            };
            let precip_note = if day.precip_mm >= 0.2 {
                format!(" 💧{:.1}", day.precip_mm)
            } else {
                String::new()
            };
            lines.push(Line::from(vec![
                Span::raw("         "),
                Span::styled(
                    format!("{} ", day.weekday),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(
                    format!("{} ", date_label),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{} ", day.icon),
                    Style::default().fg(Color::Rgb(r, g, b)),
                ),
                Span::styled(
                    format!("{:.0}°/{:.0}°", day.temp_min, day.temp_max),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(precip_note, Style::default().fg(Color::Rgb(4, 209, 249))),
            ]));
        }
    }

    lines
}

/// MCP Servers & Calls inspection face.
fn render_mcp_face(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let spinner = spinner_char(state.anim_tick);

    let Some(mcp) = &state.mcp else {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Magenta))
            .title(" MCP Servery ");
        let text = vec![
            Line::raw(""),
            Line::from(Span::styled(
                "🔌 Žádná aktivita MCP serverů",
                Style::default().fg(Color::DarkGray),
            )),
            Line::raw(""),
            Line::from("Sidebar sleduje MCP volání (Knowledge Base, LSP, OpenRouter, atd.)"),
            Line::from("přímo ze zdrojového protokolu pi relace."),
        ];
        frame.render_widget(
            Paragraph::new(text)
                .block(block)
                .scroll((state.scroll, 0))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    };

    let status_badge = if mcp.in_flight {
        Span::styled(
            format!("{} V BĚHU", spinner),
            Style::default().fg(Color::Yellow).bold(),
        )
    } else {
        Span::styled("● KLID", Style::default().fg(Color::Green).bold())
    };

    let header_line = Line::from(vec![
        Span::styled(
            "🔌 MCP Aktivita: ",
            Style::default().fg(Color::Magenta).bold(),
        ),
        status_badge,
        Span::raw("  "),
        Span::styled(
            format!("{} volání", mcp.total_calls),
            Style::default().fg(Color::White),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("📦 ~{} tok", fmt_tokens(mcp.total_tokens)),
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        if mcp.total_errors > 0 {
            Span::styled(
                format!("⚠️ {} chyb", mcp.total_errors),
                Style::default().fg(Color::Red).bold(),
            )
        } else {
            Span::styled("✓ 0 chyb", Style::default().fg(Color::Green))
        },
    ]);

    let mut lines = vec![header_line, Line::raw("")];

    // Configured / active servers
    lines.push(Line::from(Span::styled(
        "📦 Použité servery:",
        Style::default().fg(Color::Cyan).bold(),
    )));
    if mcp.servers_used.is_empty() {
        lines.push(Line::from(Span::styled(
            "   (zatím žádný server nepoužit)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let mut server_spans = vec![Span::raw("   ")];
        for s in &mcp.servers_used {
            server_spans.push(Span::styled(
                format!("[{}] ", s),
                Style::default().fg(Color::Green).bold(),
            ));
        }
        lines.push(Line::from(server_spans));
    }
    lines.push(Line::raw(""));

    // Recent calls list
    lines.push(Line::from(Span::styled(
        "⚡ Nedávná volání nástrojů:",
        Style::default().fg(Color::Cyan).bold(),
    )));
    if mcp.recent_calls.is_empty() {
        lines.push(Line::from(Span::styled(
            "   (žádná zaznamenaná volání)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for call in mcp.recent_calls.iter().rev().take(8) {
            let status = if call.is_error {
                Span::styled("✗ ", Style::default().fg(Color::Red).bold())
            } else {
                Span::styled("✓ ", Style::default().fg(Color::Green))
            };

            let badge_color = match call.badge.as_str() {
                "KB" => Color::Blue,
                "LSP" => Color::Magenta,
                "OR" => Color::Yellow,
                _ => Color::Cyan,
            };

            let mut row = vec![
                Span::raw("   "),
                status,
                Span::styled(
                    format!("[{}] ", call.badge),
                    Style::default().fg(badge_color).bold(),
                ),
                Span::styled(&call.tool, Style::default().fg(Color::White).bold()),
            ];

            if call.payload_tokens > 0 {
                row.push(Span::styled(
                    format!(" ({})", fmt_tokens(call.payload_tokens)),
                    Style::default().fg(Color::Cyan),
                ));
            }

            if !call.summary.is_empty() {
                row.push(Span::styled(
                    format!(" — {}", call.summary),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            lines.push(Line::from(row));
        }
    }

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta))
        .title(" MCP Servery & Protokol ");

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .scroll((state.scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn partition_snapshot_lines(lines: &[String]) -> (Vec<String>, Vec<String>) {
    let mut skills = Vec::new();
    let mut status = Vec::new();
    let mut in_status = false;

    for (i, line) in lines.iter().enumerate() {
        if i < 2 && (line.contains("Status") && line.contains("Skills") || line.contains("──────"))
        {
            continue;
        }

        if line.contains("📁") || line.contains("🌿") || line.contains("📊") {
            in_status = true;
        }

        if in_status {
            status.push(line.clone());
        } else {
            skills.push(line.clone());
        }
    }

    (status, skills)
}

fn parse_to_ratatui_lines(raw_lines: &[String]) -> Vec<Line<'_>> {
    let mut out = Vec::new();
    for raw in raw_lines {
        let cleaned = if let Some(stripped) = raw.strip_prefix("│ ") {
            stripped
        } else if let Some(stripped) = raw.strip_prefix("│") {
            stripped
        } else {
            raw.as_str()
        };

        match cleaned.as_bytes().into_text() {
            Ok(text) => {
                for line in text.lines {
                    out.push(line);
                }
            }
            Err(_) => {
                out.push(Line::raw(cleaned.to_string()));
            }
        }
    }
    out
}

fn render_status_face(
    frame: &mut Frame,
    area: Rect,
    snapshot: &crate::shared::PaneSnapshot,
    scroll: u16,
) {
    let (status_raw, _) = partition_snapshot_lines(&snapshot.lines);
    let chosen_raw = if !status_raw.is_empty() {
        status_raw
    } else {
        snapshot.lines.iter().skip(2).cloned().collect()
    };

    let mut parsed_lines = parse_to_ratatui_lines(&chosen_raw);

    if !snapshot.live {
        parsed_lines.insert(
            0,
            Line::from(Span::styled(
                "⟳ Pi session reloading — waiting for fresh telemetry…",
                Style::default().fg(Color::Yellow).bold(),
            )),
        );
        parsed_lines.push(Line::raw(""));
    }

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" Telemetry & Status [rev {}] ", snapshot.revision));

    let paragraph = Paragraph::new(Text::from(parsed_lines))
        .block(block)
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

/// Native Skills face rendered from the structured pi-plugin-dev sidecar:
/// header + counters, Guidance (loaded references), Focus (recent actions),
/// Gates (compliance scorecard), and a settled/running status footer.
fn render_skills_live(
    frame: &mut Frame,
    area: Rect,
    file: &crate::slices::telemetry::skills::SkillSnapshotFile,
    state: &SidebarState,
) {
    use crate::slices::telemetry::skills::{action_icon, elapsed_label, gate_badge};

    let mut lines: Vec<Line> = Vec::new();

    let Some(sk_state) = &file.state else {
        lines.push(Line::from(Span::styled(
            "🎯 Žádný aktivní skill",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "Žádný aktivní skill — načti skill (např. 'herdr-plugin-dev')",
            Style::default().fg(Color::DarkGray),
        )));
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green))
            .title(" HUD skilů & souladu ");
        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(block)
                .scroll((state.scroll, 0))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    };

    // Header: active skill + animated status indicator
    let spinner = spinner_char(state.anim_tick);
    let run_badge = if sk_state.in_turn {
        Span::styled(
            format!("{} V BĚHU", spinner),
            Style::default().fg(Color::Cyan).bold(),
        )
    } else {
        Span::styled("● KLID", Style::default().fg(Color::Green).bold())
    };

    lines.push(Line::from(vec![
        Span::styled("🎯 ", Style::default()),
        Span::styled(
            sk_state.active_skill.as_deref().unwrap_or("žádný skill"),
            Style::default().fg(Color::Yellow).bold(),
        ),
        Span::raw("  "),
        run_badge,
    ]));

    let elapsed = elapsed_label(
        sk_state
            .last_update_time
            .saturating_sub(sk_state.start_time),
    );
    lines.push(Line::from(Span::styled(
        format!(
            "   {} refů · čas: {} · {} tahů",
            sk_state.references.len(),
            elapsed,
            sk_state.turn_count
        ),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::raw(""));

    // Compliance scorecard (Gates) with visual progress bar
    let passed = sk_state
        .compliance
        .iter()
        .filter(|c| c.status == "pass")
        .count();
    let failed = sk_state
        .compliance
        .iter()
        .filter(|c| c.status == "fail")
        .count();
    let total = sk_state.compliance.len();

    let score_color = if failed > 0 {
        Color::Red
    } else if total > 0 && passed == total {
        Color::Green
    } else {
        Color::Yellow
    };

    // Mini gauge for compliance
    let (ratio, bar_str) = if total > 0 {
        let r = passed as f64 / total as f64;
        let filled = (r * 12.0).round() as usize;
        (
            r,
            format!(
                "{}{}",
                "█".repeat(filled),
                "░".repeat(12usize.saturating_sub(filled))
            ),
        )
    } else {
        (0.0, "░".repeat(12))
    };

    lines.push(Line::from(vec![
        Span::styled("🛡 Brány souladu: ", Style::default().fg(Color::Cyan).bold()),
        Span::styled(bar_str, Style::default().fg(score_color)),
        Span::styled(
            if total > 0 {
                format!(" {}/{} ({:.0}%)", passed, total, ratio * 100.0)
            } else {
                " (0)".to_string()
            },
            Style::default().fg(score_color).bold(),
        ),
    ]));

    if total == 0 {
        lines.push(Line::from(Span::styled(
            "   čekám na změny kódu k validaci…",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for c in sk_state.compliance.iter().rev().take(6) {
            let color = match c.status.as_str() {
                "pass" => Color::Green,
                "fail" => Color::Red,
                _ => Color::Yellow,
            };
            let text = if c.details.is_empty() {
                c.label.clone()
            } else {
                format!("{} — {}", c.label, c.details)
            };
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    format!("[{}] ", gate_badge(&c.status)),
                    Style::default().fg(color).bold(),
                ),
                Span::styled(text, Style::default().fg(Color::Gray)),
            ]));
        }
    }
    lines.push(Line::raw(""));

    // Loaded guidance (Pokyny)
    lines.push(Line::from(Span::styled(
        "📖 Pokyny a reference:",
        Style::default().fg(Color::Cyan).bold(),
    )));
    if sk_state.references.is_empty() {
        lines.push(Line::from(Span::styled(
            "   (zatím nenačteny žádné reference)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for r in sk_state.references.iter().rev().take(5) {
            let text = if r.summary.is_empty() {
                format!("   ✓ {}", r.name)
            } else {
                format!("   ✓ {} — {}", r.name, r.summary)
            };
            lines.push(Line::from(Span::styled(
                text,
                Style::default().fg(Color::Green),
            )));
        }
    }
    lines.push(Line::raw(""));

    // Agent focus (Poslední akce)
    lines.push(Line::from(Span::styled(
        "⚡ Fokus & akce agenta:",
        Style::default().fg(Color::Cyan).bold(),
    )));
    if sk_state.actions.is_empty() {
        lines.push(Line::from(Span::styled(
            "   (žádné nedávné akce)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for a in sk_state.actions.iter().rev().take(4) {
            let text = if a.summary.is_empty() {
                format!("   {} {}", action_icon(&a.kind), a.target)
            } else {
                format!("   {} {} — {}", action_icon(&a.kind), a.target, a.summary)
            };
            lines.push(Line::from(Span::styled(
                text,
                Style::default().fg(Color::Gray),
            )));
        }
    }
    lines.push(Line::raw(""));

    // Summary footer counters
    lines.push(Line::from(Span::styled(
        format!(
            "📊 Souhrn aktivity: {} čtení · {} zápisů kódu",
            sk_state.inspected_count, sk_state.modified_count
        ),
        Style::default().fg(Color::DarkGray),
    )));

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Green))
        .title(" HUD skilů & souladu ");

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .scroll((state.scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn render_skills_face(
    frame: &mut Frame,
    area: Rect,
    snapshot: &crate::shared::PaneSnapshot,
    scroll: u16,
) {
    let (_, skills_raw) = partition_snapshot_lines(&snapshot.lines);

    let has_content = skills_raw.iter().any(|l| {
        let t = l.replace("│", "").trim().to_string();
        !t.is_empty() && t != "Status" && t != "Skills"
    });

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Green))
        .title(format!(" HUD skilů & souladu [rev {}] ", snapshot.revision));

    if has_content {
        let mut parsed_lines = parse_to_ratatui_lines(&skills_raw);
        if !snapshot.live {
            parsed_lines.insert(
                0,
                Line::from(Span::styled(
                    "⟳ Pi session reloading — waiting for fresh telemetry…",
                    Style::default().fg(Color::Yellow).bold(),
                )),
            );
            parsed_lines.push(Line::raw(""));
        }
        let paragraph = Paragraph::new(Text::from(parsed_lines))
            .block(block)
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    } else {
        let fallback_text = vec![
            Line::raw(""),
            Line::from(Span::styled(
                "🎯 Žádný aktivní skill neběží",
                Style::default().fg(Color::Yellow).bold(),
            )),
            Line::raw(""),
            Line::from("HUD skilů zobrazuje pokyny aktivního skillu, kontrolní seznamy,"),
            Line::from("fokusní cesty a kontrolní brány, když je nějaký skill aktivní."),
            Line::raw(""),
            Line::from(vec![
                Span::styled("Tip: ", Style::default().fg(Color::Cyan).bold()),
                Span::raw("Aktivuj skill (např. 'herdr-plugin-dev' nebo 'spai-tasks') a sleduj živý průběh."),
            ]),
        ];
        let paragraph = Paragraph::new(fallback_text).block(block);
        frame.render_widget(paragraph, area);
    }
}

fn render_empty_state(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let dir = crate::shared::pi_sidebar_snapshots_dir();
    let sessions_dir = crate::shared::dirs_home()
        .map(|h| h.join(".pi").join("agent").join("sessions"))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/.pi/agent/sessions".to_string());
    let text = vec![
        Line::from(Span::styled(
            "Čekám na telemetrii pi agenta…",
            Style::default().fg(Color::Yellow).bold(),
        )),
        Line::raw(""),
        Line::from("Sidebar čte telemetrii přímo z logů pi relací:"),
        Line::from(Span::styled(sessions_dir, Style::default().fg(Color::Cyan))),
        Line::raw(""),
        Line::from("1. Spusť pi v herdr: 'pi'"),
        Line::from("2. Proved' libovolný tah — telemetrie se streamuje automaticky."),
        Line::from("3. Pro pohled Status není potřeba '/sidebar on' (snapshot"),
        Line::from("   je potřeba jen pro pohled Skilly)."),
        Line::raw(""),
        Line::from(vec![
            Span::raw("Adresář snapshotů (legacy): "),
            Span::styled(
                dir.display().to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("Cílový panel: "),
            Span::styled(
                state.target_pane_id.as_deref().unwrap_or("none"),
                Style::default().fg(Color::Magenta),
            ),
        ]),
    ];

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Telemetrie Offline ");

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_footer(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let footer_text = Line::from(vec![
        Span::styled(" [Tab/1/2/3] ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Pohled  "),
        Span::styled(" [↑/↓/j/k] ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Posun  "),
        Span::styled(" [r] ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Obnovit  "),
        Span::styled(" [q] ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Konec  "),
        Span::styled(
            format!("(Posun: {})", state.scroll),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let paragraph = Paragraph::new(footer_text);
    frame.render_widget(paragraph, area);
}
