use crate::shared::HerdrPaneInfo;
use crate::slices::telemetry::{fmt_cost, fmt_tokens};
use crate::slices::view::state::{SidebarState, Tab};
use ansi_to_tui::IntoText;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Cell, LineGauge, Paragraph, Row, Table, Tabs, Wrap},
    Frame,
};

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
}

fn render_header(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let titles = Tab::titles();
    let selected_index = state.active_tab.to_index();

    let live_indicator = if state.snapshot.as_ref().is_some_and(|s| s.live) {
        Span::styled(" ● ŽIVĚ ", Style::default().fg(Color::Green).bold())
    } else if state.snapshot.is_some() {
        // Snapshot exists but live=false: Pi session is reloading/restarting
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

    let tabs = Tabs::new(titles.to_vec())
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
        Tab::Herdr => render_herdr_panes(frame, area, &state.panes),
        Tab::Status => {
            // Prefer live telemetry parsed from the Pi session JSONL — always
            // fresher than the snapshot and independent of the TS extension.
            if let Some(t) = &state.live {
                render_live_status(frame, area, t, state.scroll);
            } else if let Some(snapshot) = &state.snapshot {
                render_status_face(frame, area, snapshot, state.scroll);
            } else {
                render_empty_state(frame, area, state);
            }
        }
        Tab::Skills => {
            // Prefer the structured sidecar (raw pi-plugin-dev state) so the
            // Rust renderer paints Gates/Focus/Guidance itself. Fall back to
            // the snapshot's pre-rendered skill lines when it's absent.
            if let Some(skills) = &state.skills {
                render_skills_live(frame, area, skills, state.scroll);
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
    scroll: u16,
) {
    let mut lines: Vec<Line> = Vec::new();

    // CWD + git badge
    if let Some(f) = &t.session_file {
        if let Some(cwd) = f.parent().map(|p| p.to_path_buf()) {
            let _ = cwd; // session folder is not the pane cwd; git shown below
        }
    }
    if !t.git_branch.is_empty() {
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

    // Context gauge (10 cells, like the statusline bar)
    if let Some(pct) = t.context_percent {
        let bar_w = 10usize;
        let filled = ((pct.min(100.0) / 100.0) * bar_w as f64).round() as usize;
        let pct_color = if pct >= 90.0 {
            Color::Red
        } else if pct >= 60.0 {
            Color::Yellow
        } else {
            Color::Green
        };
        lines.push(Line::from(vec![
            Span::styled("📊 ", Style::default()),
            Span::styled(
                format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled)),
                Style::default().fg(pct_color),
            ),
            Span::styled(
                format!(" {:.1}%", pct),
                Style::default().fg(pct_color).bold(),
            ),
            Span::styled(
                format!("/{}", fmt_tokens(t.context_window)),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(" (auto)", Style::default().fg(Color::DarkGray)),
        ]));
    }

    // Cost + token totals
    lines.push(Line::from(vec![
        Span::styled(
            format!("💰 {}", fmt_cost(t.total_cost)),
            Style::default().fg(Color::Yellow),
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
    ]));

    // Cache
    let prompt = t.input_tokens + t.cache_read + t.cache_write;
    if prompt > 0 {
        let hit_pct = (t.cache_read as f64 / prompt as f64) * 100.0;
        lines.push(Line::from(vec![
            Span::styled("📦 ", Style::default()),
            Span::styled(fmt_tokens(t.cache_read), Style::default().fg(Color::Gray)),
            Span::styled(
                format!(" (w:{}) ", fmt_tokens(t.cache_write)),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("🎯{:.0}%", hit_pct),
                Style::default().fg(Color::Green),
            ),
        ]));
    }

    // Model + provider + thinking
    if !t.model_id.is_empty() {
        let mut model_line = vec![
            Span::styled(
                format!("({}) ", t.provider),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(t.model_id.clone(), Style::default().fg(Color::Green).bold()),
        ];
        if !t.thinking_level.is_empty() {
            model_line.push(Span::styled(" • 🧠 ", Style::default().fg(Color::DarkGray)));
            model_line.push(Span::styled(
                t.thinking_level.clone(),
                Style::default().fg(Color::Cyan),
            ));
        }
        lines.push(Line::from(model_line));
    }

    // Footer diagnostics: session id + freshness
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("⚡ živá relace ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            t.session_id.chars().take(8).collect::<String>(),
            Style::default().fg(Color::Magenta),
        ),
    ]));

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Telemetrie & Stav [živě] ");

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn partition_snapshot_lines(lines: &[String]) -> (Vec<String>, Vec<String>) {
    let mut skills = Vec::new();
    let mut status = Vec::new();
    let mut in_status = false;

    for (i, line) in lines.iter().enumerate() {
        // Skip first 2 lines if they are old ASCII tab headers
        if i < 2 && (line.contains("Status") && line.contains("Skills") || line.contains("──────"))
        {
            continue;
        }

        // Status telemetry marker: starts at CWD / branch / context bar
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
        // Fallback to all non-tab-bar lines
        snapshot.lines.iter().skip(2).cloned().collect()
    };

    let mut parsed_lines = parse_to_ratatui_lines(&chosen_raw);

    if !snapshot.live {
        // /reload in progress: banner on top of the retained frame so the user
        // sees the header/sidebar is about to refresh, not stale data.
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
    scroll: u16,
) {
    use crate::slices::telemetry::skills::{action_icon, elapsed_label, gate_badge};

    let mut lines: Vec<Line> = Vec::new();

    let Some(state) = &file.state else {
        lines.push(Line::from(Span::styled(
            "🎯 no skill",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "No active skill — activate one (e.g. 'herdr-plugin-dev')",
            Style::default().fg(Color::DarkGray),
        )));
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green))
            .title(" HUD skilů & souladu ");
        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(block)
                .scroll((scroll, 0))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    };

    // Header: active skill + counters
    lines.push(Line::from(Span::styled(
        format!("🎯 {}", state.active_skill.as_deref().unwrap_or("žádný skill")),
        Style::default().fg(Color::Yellow).bold(),
    )));
    let elapsed = elapsed_label(state.last_update_time.saturating_sub(state.start_time));
    lines.push(Line::from(Span::styled(
        format!(
            "{} refů · {} · {} tahů",
            state.references.len(),
            elapsed,
            state.turn_count
        ),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::raw(""));

    // Loaded guidance
    lines.push(Line::from(Span::styled(
        "📖 Pokyny",
        Style::default().fg(Color::Cyan),
    )));
    if state.references.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (zatím nic)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for r in state.references.iter().rev().take(6) {
            let text = if r.summary.is_empty() {
                format!("  ✓ {}", r.name)
            } else {
                format!("  ✓ {} — {}", r.name, r.summary)
            };
            lines.push(Line::from(Span::styled(
                text,
                Style::default().fg(Color::Green),
            )));
        }
    }
    lines.push(Line::raw(""));

    // Agent focus — most recent actions
    lines.push(Line::from(Span::styled(
        "⚡ Fokus",
        Style::default().fg(Color::Cyan),
    )));
    if state.actions.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (nečinný)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for a in state.actions.iter().rev().take(3) {
            let text = if a.summary.is_empty() {
                format!("  {} {}", action_icon(&a.kind), a.target)
            } else {
                format!("  {} {} — {}", action_icon(&a.kind), a.target, a.summary)
            };
            lines.push(Line::from(Span::styled(
                text,
                Style::default().fg(Color::Gray),
            )));
        }
    }
    lines.push(Line::raw(""));

    // Compliance scorecard (Gates)
    let passed = state
        .compliance
        .iter()
        .filter(|c| c.status == "pass")
        .count();
    let failed = state
        .compliance
        .iter()
        .filter(|c| c.status == "fail")
        .count();
    let total = state.compliance.len();
    let score_text = if total > 0 {
        format!("{}/{}", passed, total)
    } else {
        "n/a".to_string()
    };
    let score_color = if failed > 0 {
        Color::Red
    } else if total > 0 && passed == total {
        Color::Green
    } else {
        Color::DarkGray
    };
    lines.push(Line::from(vec![
        Span::styled("🛡 ", Style::default()),
        Span::styled("Brány ", Style::default().fg(Color::Cyan)),
        Span::styled(score_text, Style::default().fg(score_color).bold()),
    ]));

    if total == 0 {
        lines.push(Line::from(Span::styled(
            "  čekám na změny kódu",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for c in state.compliance.iter().rev().take(6) {
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
                Span::styled(
                    format!("[{}] ", gate_badge(&c.status)),
                    Style::default().fg(color).bold(),
                ),
                Span::styled(text, Style::default().fg(Color::Gray)),
            ]));
        }
    }
    lines.push(Line::raw(""));

    // Status footer
    if state.in_turn {
        lines.push(Line::from(Span::styled(
            "● běží",
            Style::default().fg(Color::Cyan),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!(
                "✓ klid · {} čtení · {} zápisů",
                state.inspected_count, state.modified_count
            ),
            Style::default().fg(Color::Green),
        )));
    }

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Green))
        .title(" HUD skilů & souladu ");

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .scroll((scroll, 0))
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
        .title(format!(
            " HUD skilů & souladu [rev {}] ",
            snapshot.revision
        ));

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
            Line::from(Span::styled("🎯 Žádný aktivní skill neběží", Style::default().fg(Color::Yellow).bold())),
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
        Line::raw(""),
        Line::from(vec![
            Span::styled("Tip: ", Style::default().fg(Color::Green).bold()),
            Span::raw("Stiskni [3] nebo Tab a prohlédni herdr panely v tomto workspace."),
        ]),
    ];

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Telemetrie Offline ");

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_herdr_panes(frame: &mut Frame, area: Rect, panes: &[HerdrPaneInfo]) {
    let header = Row::new(vec![
        Cell::from("ID panelu").style(Style::default().fg(Color::Cyan).bold()),
        Cell::from("Tab").style(Style::default().fg(Color::Cyan).bold()),
        Cell::from("Workspace").style(Style::default().fg(Color::Cyan).bold()),
        Cell::from("Popisek").style(Style::default().fg(Color::Cyan).bold()),
        Cell::from("Aktivní").style(Style::default().fg(Color::Cyan).bold()),
        Cell::from("CWD").style(Style::default().fg(Color::Cyan).bold()),
    ])
    .bottom_margin(1);

    let rows: Vec<Row> = panes
        .iter()
        .map(|p| {
            let is_focused = p.focused.unwrap_or(false);
            let focus_cell = if is_focused {
                Cell::from("● ANO").style(Style::default().fg(Color::Green).bold())
            } else {
                Cell::from("○ ne").style(Style::default().fg(Color::DarkGray))
            };

            Row::new(vec![
                Cell::from(p.pane_id.clone()).style(Style::default().fg(Color::Yellow)),
                Cell::from(p.tab_id.clone().unwrap_or_else(|| "-".to_string())),
                Cell::from(p.workspace_id.clone().unwrap_or_else(|| "-".to_string())),
                Cell::from(p.label.clone().unwrap_or_else(|| "-".to_string())),
                focus_cell,
                Cell::from(p.cwd.clone().unwrap_or_else(|| "-".to_string()))
                    .style(Style::default().dim()),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Length(8),
        Constraint::Fill(1),
    ];

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta))
        .title(format!(" Herdr panely workspaceu ({}) ", panes.len()));

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .column_spacing(1);

    frame.render_widget(table, area);
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
