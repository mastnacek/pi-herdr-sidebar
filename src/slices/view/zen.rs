//! Zen face — serene git, skill, MCP, SPAI, and weather overview.
use crate::slices::telemetry::fmt_tokens;
use crate::slices::view::state::SidebarState;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Paragraph, Wrap},
    Frame,
};

/// Serene, low-dopamine Zen view: secondary domains (Git, Skill, MCP, SPAI, Weather).
pub fn render_zen_face(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let mut lines: Vec<Line> = Vec::new();

    let live = state.live.as_ref();
    let mcp = state.mcp.as_ref();
    let skill_state = state.skills.as_ref().and_then(|s| s.state.as_ref());

    lines.push(Line::raw(""));

    // 1. Git Overview
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

    // 2. Active skill
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

    // 3. MCP status
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

    // 4. SPAI tasks
    lines.push(Line::raw(""));
    lines.extend(super::spai_ui::render_spai_lines(state));

    // 5. Weather (yr.no Locationforecast 2.0)
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
