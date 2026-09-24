use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span},
};

use crate::slices::telemetry::openrouter_live::OpenRouterCreditTelemetry;

fn credit_color(remaining: f64) -> Color {
    if remaining > 10.0 {
        Color::Rgb(95, 200, 140) // Mint green (> $10)
    } else if remaining > 2.0 {
        Color::Rgb(230, 200, 90) // Amber / yellow ($2 - $10)
    } else {
        Color::Rgb(241, 108, 117) // Coral red (< $2)
    }
}

/// Render OpenRouter credits line(s) for Zen view (directly under Náklady/Costs item).
/// Highlighted in color as it is important data.
pub fn render_zen_openrouter_lines(credits: &OpenRouterCreditTelemetry) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if credits.accounts.is_empty() {
        return lines;
    }

    if credits.accounts.len() == 1 {
        let acc = &credits.accounts[0];
        let col = credit_color(acc.remaining_credits);
        lines.push(Line::from(vec![
            Span::styled("Kredity: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("${:.2}", acc.remaining_credits),
                Style::default().fg(col).bold(),
            ),
            Span::styled(
                format!(
                    " (použito ${:.2} z ${:.2})",
                    acc.total_usage, acc.total_credits
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![Span::styled(
            "Kredity OR:",
            Style::default().fg(Color::Cyan).bold(),
        )]));
        for acc in &credits.accounts {
            let col = credit_color(acc.remaining_credits);
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{}: ", acc.label),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("${:.2}", acc.remaining_credits),
                    Style::default().fg(col).bold(),
                ),
                Span::styled(
                    format!(
                        " (použito ${:.2} z ${:.2})",
                        acc.total_usage, acc.total_credits
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    lines
}

/// Render OpenRouter credits section for Status view.
pub fn render_status_openrouter_lines(credits: &OpenRouterCreditTelemetry) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if credits.accounts.is_empty() {
        return lines;
    }

    let mut title_spans = vec![Span::styled(
        "💳 Kredity OpenRouter: ",
        Style::default().fg(Color::Yellow).bold(),
    )];

    if credits.accounts.len() == 1 {
        let acc = &credits.accounts[0];
        let col = credit_color(acc.remaining_credits);
        title_spans.push(Span::styled(
            format!("${:.2} zbývá", acc.remaining_credits),
            Style::default().fg(col).bold(),
        ));
        title_spans.push(Span::styled(
            format!(
                " │ vyčerpáno ${:.2} / celkem ${:.2}",
                acc.total_usage, acc.total_credits
            ),
            Style::default().fg(Color::DarkGray),
        ));
        lines.push(Line::from(title_spans));
    } else {
        lines.push(Line::from(title_spans));
        for acc in &credits.accounts {
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
                        "(vyčerpáno ${:.2} z ${:.2})",
                        acc.total_usage, acc.total_credits
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    lines
}
