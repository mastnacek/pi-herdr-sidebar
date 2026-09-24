use crate::slices::view::state::{SidebarState, Tab};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span},
    widgets::{Block, BorderType, LineGauge, Paragraph, Tabs},
    Frame,
};

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub(super) fn spinner_char(tick: u64) -> &'static str {
    SPINNER_FRAMES[(tick as usize) % SPINNER_FRAMES.len()]
}

pub fn render(frame: &mut Frame, state: &SidebarState) {
    let area = frame.area();
    let banner_h = super::shared_banner::shared_banner_height(state);

    if state.refresh_timer > 0 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Length(banner_h),
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
        super::shared_banner::render_shared_model_banner(frame, chunks[2], state);
        render_body(frame, chunks[3], state);
        render_footer(frame, chunks[4], state);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(banner_h),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
            .split(area);

        render_header(frame, chunks[0], state);
        super::shared_banner::render_shared_model_banner(frame, chunks[1], state);
        render_body(frame, chunks[2], state);
        render_footer(frame, chunks[3], state);
    }
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
        Tab::Zen => super::zen::render_zen_face(frame, area, state),
        Tab::Mcp => super::mcp::render_mcp_face(frame, area, state),
        Tab::Status => {
            if let Some(t) = &state.live {
                super::status::render_live_status(frame, area, t, state);
            } else if let Some(snapshot) = &state.snapshot {
                super::status::render_status_face(frame, area, snapshot, state.scroll);
            } else {
                render_empty_state(frame, area, state);
            }
        }
        Tab::Skills => {
            if let Some(skills) = &state.skills {
                super::skills::render_skills_live(frame, area, skills, state);
            } else if let Some(snapshot) = &state.snapshot {
                super::skills::render_skills_face(frame, area, snapshot, state.scroll);
            } else {
                render_empty_state(frame, area, state);
            }
        }
    }
}

// render_live_status → status.rs; zen → zen.rs; spai → spai_ui.rs; weather → weather_ui.rs
// render_mcp_face → mcp.rs; skills faces → skills.rs; snapshot faces → status.rs
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
        Span::styled(" [w] ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Lokalita  "),
        Span::styled(" [c] ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Kopírovat předpověď  "),
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
