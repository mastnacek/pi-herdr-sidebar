use super::ui_chrome::{render_empty_state, render_footer};
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
                Constraint::Fill(1),
                Constraint::Length(banner_h),
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
        super::shared_banner::render_shared_model_banner(frame, chunks[3], state);
        render_footer(frame, chunks[4], state);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Fill(1),
                Constraint::Length(banner_h),
                Constraint::Length(1),
            ])
            .split(area);

        render_header(frame, chunks[0], state);
        render_body(frame, chunks[1], state);
        super::shared_banner::render_shared_model_banner(frame, chunks[2], state);
        render_footer(frame, chunks[3], state);
    }

    // Fullscreen Scratchpad modal (Ctrl+N) sits above everything.
    if state.spai_notes.scratch.open {
        crate::slices::spai_notes::scratch::view::render_scratch(frame, area, &state.spai_notes);
    }
}

fn render_header(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let selected_index = state.active_tab.to_index();
    let spinner = spinner_char(state.anim_tick);

    let is_zen = state.active_tab == Tab::Zen;

    // 0. Zen tab: calm, serene indicator
    let zen_spans = vec![Span::raw(super::state_model::TAB_LABELS[0])];

    // 1. Status tab indicator: spinner ONLY when agent is actively working/executing (and not in Zen tab)
    let is_agent_working = state.live.as_ref().map(|l| l.is_working).unwrap_or(false);
    let status_spans = if is_agent_working && !is_zen {
        vec![
            Span::raw(super::state_model::TAB_LABELS[1]),
            Span::styled(spinner, Style::default().fg(Color::Cyan).bold()),
            Span::raw(" "),
        ]
    } else {
        vec![Span::raw(super::state_model::TAB_LABELS[1])]
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
            Span::raw(super::state_model::TAB_LABELS[2]),
            Span::styled(spinner, Style::default().fg(Color::Yellow).bold()),
            Span::raw(" "),
        ]
    } else {
        vec![Span::raw(super::state_model::TAB_LABELS[2])]
    };

    // 3. MCP tab indicator: spinner when MCP calls are in flight (and not in Zen tab)
    let is_mcp_in_flight = state.mcp.as_ref().map(|m| m.in_flight).unwrap_or(false);
    let mcp_spans = if is_mcp_in_flight && !is_zen {
        vec![
            Span::raw(super::state_model::TAB_LABELS[3]),
            Span::styled(spinner, Style::default().fg(Color::Magenta).bold()),
            Span::raw(" "),
        ]
    } else {
        vec![Span::raw(super::state_model::TAB_LABELS[3])]
    };

    // 4. Notes tab indicator (SPAI notes & fileviewer)
    let notes_spans = vec![Span::raw(super::state_model::TAB_LABELS[4])];

    // 5. Shortcuts tab (keybindings).
    let shortcuts_spans = vec![Span::raw(super::state_model::TAB_LABELS[5])];

    // 6. Settings tab (API key, models, vectorization)
    let settings_spans = vec![Span::raw(super::state_model::TAB_LABELS[6])];

    let titles: Vec<Line> = vec![
        Line::from(zen_spans),
        Line::from(status_spans),
        Line::from(skills_spans),
        Line::from(mcp_spans),
        Line::from(notes_spans),
        Line::from(shortcuts_spans),
        Line::from(settings_spans),
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
        Tab::Notes => {
            crate::slices::spai_notes::render_spai_notes_tab(frame, area, &state.spai_notes)
        }
        Tab::Shortcuts => {
            crate::slices::shortcuts::render_shortcuts_tab(frame, area, &state.shortcuts)
        }
        Tab::Settings => {
            crate::slices::settings::render_settings_tab(frame, area, &state.settings)
        }
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
use crate::slices::view::state::{SidebarState, Tab};
