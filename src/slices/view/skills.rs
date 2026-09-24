//! Skills face — structured sidecar HUD + snapshot fallback.
use super::status::{parse_to_ratatui_lines, partition_snapshot_lines};
use super::ui::spinner_char;
use crate::slices::view::state::SidebarState;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};
use ratatui::Frame;

/// Native Skills face rendered from the structured pi-plugin-dev sidecar:
/// header + counters, Guidance (loaded references), Focus (recent actions),
/// Gates (compliance scorecard), and a settled/running status footer.
pub fn render_skills_live(
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

pub fn render_skills_face(
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
