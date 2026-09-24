//! Weather face lines — yr.no Locationforecast 2.0.
use crate::slices::view::state::SidebarState;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};

/// Source: MET Norway (yr.no) Locationforecast 2.0 — no API key, User-Agent identified.
pub fn render_weather_lines(state: &SidebarState) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let Some(w) = &state.weather else {
        lines.push(Line::from(vec![
            Span::styled("Počasí:   ", Style::default().fg(Color::DarkGray)),
            Span::styled("načítám…", Style::default().fg(Color::DarkGray)),
        ]));
        return lines;
    };

    if let Some(err) = &w.error {
        lines.push(Line::from(vec![
            Span::styled("Počasí:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(err.clone(), Style::default().fg(Color::Red)),
        ]));
        return lines;
    }

    // Header: location + selected marker + key hint
    let loc_name = w.location_name.clone();
    let idx = w.location_index;
    let total = crate::slices::telemetry::weather_live::LOCATIONS.len();
    lines.push(Line::from(vec![
        Span::styled("Počasí:   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("[{} / {}] ", idx + 1, total),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(loc_name, Style::default().fg(Color::Cyan).bold()),
        Span::styled("  (w=rotovat)", Style::default().fg(Color::DarkGray)),
    ]));

    // 7-day forecast: today first — uniform detail rows for all 7 days
    if !w.days.is_empty() {
        lines.push(Line::from(Span::styled(
            "Týden:    ".to_string(),
            Style::default().fg(Color::DarkGray),
        )));
        for day in &w.days {
            let (r, g, b) = day.color;
            // "2026-09-25" → "25.9."
            let date_parts: Vec<&str> = day.date.split('-').collect();
            let date_label = if date_parts.len() == 3 {
                let m = date_parts[1].trim_start_matches('0');
                let d = date_parts[2].trim_start_matches('0');
                format!("{}.{}. ", d, m)
            } else {
                String::new()
            };
            let precip_note = if day.precip_mm >= 0.2 {
                format!(" srážky {:.1} l/m²", day.precip_mm)
            } else {
                String::new()
            };
            lines.push(Line::from(vec![
                Span::raw("         "),
                Span::styled(
                    format!("{} ", day.weekday),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(
                    format!("{} ", date_label),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{} ", day.icon),
                    Style::default().fg(Color::Rgb(r, g, b)),
                ),
                Span::styled(
                    format!("{:.0}°/{:.0}°", day.temp_min, day.temp_max),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(precip_note, Style::default().fg(Color::Rgb(4, 209, 249))),
            ]));
        }
    }

    lines
}
