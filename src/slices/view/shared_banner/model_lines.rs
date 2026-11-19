//! Model, thinking level and turn/tool counters for the shared banner.

use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span},
};

use super::super::ui::spinner_char;
use crate::slices::telemetry::LiveTelemetry;
use crate::slices::view::state::SidebarState;

/// Code Mode badge (Claude panes only): bright when the transcript proves it,
/// dim when only the global setting says so. `🧩` — `⚡` is taken by the turns line.
pub(super) fn code_mode_badge(l: &LiveTelemetry) -> Option<Span<'static>> {
    use crate::slices::telemetry::claude_session::code_mode::CodeMode;
    match l.code_mode {
        CodeMode::Off => None,
        CodeMode::Transcript => Some(Span::styled("🧩 CM", Style::default().fg(Color::Yellow).bold())),
        CodeMode::Config => Some(Span::styled("🧩 CM?", Style::default().fg(Color::DarkGray))),
    }
}

pub(super) fn build_model_and_turns_lines(state: &SidebarState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let live = state.live.as_ref();
    let spinner = spinner_char(state.anim_tick);
    let is_working = live.map(|l| l.is_working).unwrap_or(false);
    let status_icon = if is_working {
        Span::styled(
            format!("{} ", spinner),
            Style::default().fg(Color::Cyan).bold(),
        )
    } else {
        Span::styled("● ", Style::default().fg(Color::Green))
    };

    let provider = live
        .map(|l| l.provider.as_str())
        .filter(|p| !p.is_empty())
        .unwrap_or("pi");

    let model_id = live
        .map(|l| {
            if l.model_id.is_empty() {
                "neznámý model".to_string()
            } else {
                l.model_id.clone()
            }
        })
        .unwrap_or_else(|| "offline".to_string());

    let mut model_line = vec![
        status_icon,
        Span::styled(
            format!("({}) ", provider),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(model_id, Style::default().fg(Color::Green).bold()),
    ];

    if let Some(l) = live {
        if !l.thinking_level.is_empty() {
            model_line.push(Span::styled(" • 🧠 ", Style::default().fg(Color::DarkGray)));
            model_line.push(Span::styled(
                l.thinking_level.clone(),
                Style::default().fg(Color::Cyan),
            ));
        }
    }
    if let Some(badge) = live.and_then(code_mode_badge) {
        model_line.push(Span::raw(" "));
        model_line.push(badge);
    }
    lines.push(Line::from(model_line));

    if let Some(l) = live {
        lines.push(Line::from(vec![
            Span::styled("  ⚡ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{} tahů", l.turns_count),
                Style::default().fg(Color::White),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} nástrojů", l.tool_calls_count),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            if l.tool_errors_count > 0 {
                Span::styled(
                    format!("⚠️ {} chyb", l.tool_errors_count),
                    Style::default().fg(Color::Red).bold(),
                )
            } else {
                Span::styled("✓ 0 chyb", Style::default().fg(Color::Green))
            },
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  ⚡ ", Style::default().fg(Color::DarkGray)),
            Span::styled("čekám na aktivitu…", Style::default().fg(Color::DarkGray)),
        ]));
    }
    lines
}
