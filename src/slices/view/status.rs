//! Status faces — git tree, session tokens/cache, quota, diagnostics.
use crate::slices::telemetry::fmt_tokens;
use crate::slices::view::state::SidebarState;
use ansi_to_tui::IntoText;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};
use ratatui::Frame;

/// Native statusline-parity face rendered straight from LiveTelemetry:
/// Git working tree, cache breakdown, session timing, and detailed diagnostics.
pub fn render_live_status(
    frame: &mut Frame,
    area: Rect,
    t: &crate::slices::telemetry::LiveTelemetry,
    state: &SidebarState,
) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::raw(""));

    // 1. Git Status Section
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

    // 1b. Recent commits protocol (last 3 commits)
    if let Some(git) = &t.git {
        if !git.recent_commits.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![Span::styled(
                "📜 Poslední commity:",
                Style::default().fg(Color::Cyan).bold(),
            )]));
            for c in git.recent_commits.iter().take(3) {
                lines.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(&c.hash, Style::default().fg(Color::Magenta).bold()),
                    Span::styled(
                        format!(" ({}) ", c.age),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        if c.message.chars().count() > 34 {
                            let s: String = c.message.chars().take(32).collect();
                            format!("{}…", s)
                        } else {
                            c.message.clone()
                        },
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            }
        }

        // 1c. Monorepo mode: nested repos discovered from session edit trail
        if !git.touched_repos.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![Span::styled(
                "📦 Změněné repozitáře:",
                Style::default().fg(Color::Cyan).bold(),
            )]));
            for repo in git.touched_repos.iter().take(3) {
                let edited_tag = if repo.touched_files > 0 {
                    format!(" ●{}", repo.touched_files)
                } else {
                    String::new()
                };
                lines.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        &repo.name,
                        Style::default().fg(Color::White).bold(),
                    ),
                    Span::styled(edited_tag, Style::default().fg(Color::Yellow)),
                ]));
                for c in repo.recent_commits.iter().take(2) {
                    lines.push(Line::from(vec![
                        Span::styled("      ", Style::default()),
                        Span::styled(&c.hash, Style::default().fg(Color::Magenta)),
                        Span::styled(
                            format!(" ({}) ", c.age),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            if c.message.chars().count() > 30 {
                                let s: String = c.message.chars().take(28).collect();
                                format!("{}…", s)
                            } else {
                                c.message.clone()
                            },
                            Style::default().fg(Color::Gray),
                        ),
                    ]));
                }
            }
        }
    }
    lines.push(Line::raw(""));

    // 2. Sliding window token consumption
    if let Some(q) = &state.quota {
        let slide_spans = vec![
            Span::styled("⏳ Posuvné okno:", Style::default().fg(Color::Cyan).bold()),
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
        .title(" Detailní stav & Git ");

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
