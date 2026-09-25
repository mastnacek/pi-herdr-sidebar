//! Context-window gauge and token economics/cache lines for the shared banner.
//!
//! The gauge renders `?` whenever pi reports the context as unknown
//! (post-compaction gap, or no window resolved) instead of a fake `0.0%`.

use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span},
};

use crate::slices::telemetry::{fmt_cost, fmt_tokens};
use crate::slices::view::state::SidebarState;

pub(super) fn build_context_and_cost_lines(state: &SidebarState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let live = state.live.as_ref();
    let ctx_tokens = live.map(|l| l.context_tokens).unwrap_or(0);
    let ctx_window = live.map(|l| l.context_window).unwrap_or(0);
    let ctx_pct = live.and_then(|l| l.context_percent).unwrap_or(0.0);
    // pi reports `?` while the value is unknown (e.g. right after a compaction)
    // and there is nothing to draw a bar from without a known window.
    let ctx_known = live.is_some_and(|l| !l.context_unknown) && ctx_window > 0;

    let bar_w = 16usize;
    let filled = if ctx_known {
        ((ctx_pct.min(100.0) / 100.0) * bar_w as f64).round() as usize
    } else {
        0
    };
    let pct_color = if !ctx_known {
        Color::DarkGray
    } else if ctx_pct >= 90.0 {
        Color::Red
    } else if ctx_pct >= 60.0 {
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
            if ctx_known {
                format!(" {:.1}%", ctx_pct)
            } else {
                " ?".to_string()
            },
            Style::default().fg(pct_color).bold(),
        ),
        Span::styled(
            if ctx_known {
                format!(" ({}/{})", fmt_tokens(ctx_tokens), fmt_tokens(ctx_window))
            } else if ctx_window > 0 {
                format!(" (?/{})", fmt_tokens(ctx_window))
            } else {
                " (neznámé okno)".to_string()
            },
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    if let Some(l) = live {
        lines.push(Line::from(vec![
            Span::styled(
                format!("💰 {}", fmt_cost(l.total_cost)),
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("⬆️ {}", fmt_tokens(l.input_tokens)),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("⬇️ {}", fmt_tokens(l.output_tokens)),
                Style::default().fg(Color::Green),
            ),
            if l.reasoning_tokens > 0 {
                Span::styled(
                    format!(" │ 🧠 {}", fmt_tokens(l.reasoning_tokens)),
                    Style::default().fg(Color::Magenta),
                )
            } else {
                Span::raw("")
            },
        ]));

        let prompt = l.input_tokens + l.cache_read + l.cache_write;
        if prompt > 0 && l.cache_read > 0 {
            let hit_pct = (l.cache_read as f64 / prompt as f64) * 100.0;
            lines.push(Line::from(vec![
                Span::styled("📦 Mezipaměť: ", Style::default().fg(Color::Cyan).bold()),
                Span::styled(
                    format!("čtení: {} ", fmt_tokens(l.cache_read)),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(
                    format!("zápis: {} ", fmt_tokens(l.cache_write)),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("(🎯 úspora {:.0}%)", hit_pct),
                    Style::default().fg(Color::Green).bold(),
                ),
            ]));
        }
    } else {
        lines.push(Line::from(vec![Span::styled(
            "💰 $0.00",
            Style::default().fg(Color::DarkGray),
        )]));
    }
    lines
}
