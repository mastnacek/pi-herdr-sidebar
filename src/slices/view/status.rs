//! Status faces — native LiveTelemetry statusline-parity + snapshot fallback.
use super::ui::spinner_char;
use crate::slices::telemetry::{fmt_cost, fmt_tokens};
use crate::slices::view::state::SidebarState;
use ansi_to_tui::IntoText;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};
use ratatui::Frame;

/// Native statusline-parity face rendered straight from LiveTelemetry.
pub fn render_live_status(
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

    if let Some(or_credits) = &state.openrouter_credits {
        lines.push(Line::raw(""));
        lines.extend(super::openrouter_ui::render_status_openrouter_lines(
            or_credits,
        ));
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

pub(super) fn partition_snapshot_lines(lines: &[String]) -> (Vec<String>, Vec<String>) {
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

pub(super) fn parse_to_ratatui_lines(raw_lines: &[String]) -> Vec<Line<'_>> {
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

pub fn render_status_face(
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
