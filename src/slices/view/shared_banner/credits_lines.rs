//! Quota and credit lines (Antigravity caps, OpenRouter account balances).

use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span},
};

use crate::slices::view::state::SidebarState;

pub(super) fn credit_color(remaining: f64) -> Color {
    if remaining > 10.0 {
        Color::Rgb(95, 200, 140)
    } else if remaining > 2.0 {
        Color::Rgb(230, 200, 90)
    } else {
        Color::Rgb(241, 108, 117)
    }
}

pub(super) fn build_quota_and_credits_lines(state: &SidebarState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let Some(anti) = state.quota.as_ref().and_then(|q| q.antigravity.as_ref()) {
        let col_lavender = Color::Rgb(170, 160, 220);
        let col_dim = Color::Rgb(120, 124, 140);
        let cap_col = |p: u32| {
            if p > 35 {
                Color::Rgb(95, 200, 140)
            } else if p > 15 {
                Color::Rgb(230, 200, 90)
            } else {
                Color::Rgb(241, 108, 117)
            }
        };

        let mut spans = vec![Span::styled(
            "🪐 Antigravity: ",
            Style::default().fg(Color::Rgb(95, 200, 230)).bold(),
        )];
        let mut added = false;
        if let Some(pct5) = anti.five_hour_pct {
            spans.push(Span::styled("5h ", Style::default().fg(col_lavender)));
            spans.push(Span::styled(
                format!("{}%", pct5),
                Style::default().fg(cap_col(pct5)).bold(),
            ));
            if let Some(t5) = &anti.five_hour_time {
                spans.push(Span::styled(
                    format!(" ({})", t5),
                    Style::default().fg(col_dim),
                ));
            }
            added = true;
        }
        if let Some(pct_wk) = anti.weekly_pct {
            if added {
                spans.push(Span::styled(" · ", Style::default().fg(col_dim)));
            }
            spans.push(Span::styled("Wk ", Style::default().fg(col_lavender)));
            spans.push(Span::styled(
                format!("{}%", pct_wk),
                Style::default().fg(cap_col(pct_wk)).bold(),
            ));
            if let Some(tw) = &anti.weekly_time {
                spans.push(Span::styled(
                    format!(" ({})", tw),
                    Style::default().fg(col_dim),
                ));
            }
        }
        lines.push(Line::from(spans));
    }

    if let Some(or) = &state.openrouter_credits {
        if or.accounts.len() == 1 {
            let acc = &or.accounts[0];
            let col = credit_color(acc.remaining_credits);
            lines.push(Line::from(vec![
                Span::styled("💳 Kredity: ", Style::default().fg(Color::Yellow).bold()),
                Span::styled(
                    format!("${:.2} zbývá", acc.remaining_credits),
                    Style::default().fg(col).bold(),
                ),
                Span::styled(
                    format!(
                        " (vyčerpáno ${:.2} / ${:.2})",
                        acc.total_usage, acc.total_credits
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        } else if or.accounts.len() > 1 {
            lines.push(Line::from(vec![Span::styled(
                "💳 Kredity OpenRouter:",
                Style::default().fg(Color::Yellow).bold(),
            )]));
            for acc in &or.accounts {
                let col = credit_color(acc.remaining_credits);
                lines.push(Line::from(vec![
                    Span::styled("   ● ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{}: ", acc.label),
                        Style::default().fg(Color::White).bold(),
                    ),
                    Span::styled(
                        format!("${:.2} zbývá ", acc.remaining_credits),
                        Style::default().fg(col).bold(),
                    ),
                    Span::styled(
                        format!(
                            "(vyčerpáno ${:.2} / ${:.2})",
                            acc.total_usage, acc.total_credits
                        ),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }
    }
    lines
}
