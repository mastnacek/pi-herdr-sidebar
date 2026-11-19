//! Weather face lines — yr.no Locationforecast 2.0.
use crate::slices::telemetry::weather_live::DayForecast;
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

    // 7-day forecast, today first: one fixed-width table row per day.
    if !w.days.is_empty() {
        lines.push(Line::from(Span::styled(
            "Týden:    ".to_string(),
            Style::default().fg(Color::DarkGray),
        )));
        for (i, day) in w.days.iter().enumerate() {
            lines.push(day_row(day, i == 0));
        }
    }

    lines
}

/// `  Po  24.11.   −3° …  4°   2,1 mm  🌧`
///
/// Every column has a fixed width and the weather icon comes last: emoji
/// render one or two cells wide depending on the glyph and the terminal
/// (`☀ ☁ ❄` vs `🌧 ⛅`), so anything after the icon would drift. Rain shows
/// only from 0,2 mm; the column stays blank otherwise so the icons line up.
fn day_row(day: &DayForecast, today: bool) -> Line<'static> {
    let (r, g, b) = day.color;
    let weekday = if today {
        Style::default().fg(Color::White).bold()
    } else {
        Style::default().fg(Color::Gray)
    };
    let rain = if day.precip_mm >= 0.2 {
        format!("{:>7}", format!("{:.1} mm", day.precip_mm).replace('.', ","))
    } else {
        " ".repeat(7)
    };
    Line::from(vec![
        Span::raw("  "),
        Span::styled(format!("{:<2}", day.weekday), weekday),
        Span::styled(format!(" {:>6}", date_label(&day.date)), Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" {:>5}", temp(day.temp_min)), Style::default().fg(temp_color(day.temp_min))),
        Span::styled(" … ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:>4}", temp(day.temp_max)), Style::default().fg(temp_color(day.temp_max))),
        Span::styled(format!(" {rain}"), Style::default().fg(Color::Rgb(4, 209, 249))),
        Span::styled(format!("  {}", day.icon), Style::default().fg(Color::Rgb(r, g, b))),
    ])
}

/// `2026-09-05` → `5.9.`
fn date_label(date: &str) -> String {
    match date.split('-').collect::<Vec<_>>().as_slice() {
        [_, m, d] => format!("{}.{}.", d.trim_start_matches('0'), m.trim_start_matches('0')),
        _ => String::new(),
    }
}

/// Rounded, with a real minus sign (`−3°`) and never `-0°`.
fn temp(t: f64) -> String {
    let n = t.round() as i64;
    if n < 0 {
        format!("−{}°", -n)
    } else {
        format!("{n}°")
    }
}

/// Frost blue, cool cyan, mild gray, warm yellow, hot orange.
fn temp_color(t: f64) -> Color {
    match t.round() as i64 {
        i64::MIN..=-5 => Color::Rgb(90, 140, 255),
        -4..=0 => Color::Rgb(140, 200, 255),
        1..=14 => Color::Gray,
        15..=24 => Color::Rgb(241, 220, 110),
        _ => Color::Rgb(255, 150, 80),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(weekday: &str, date: &str, min: f64, max: f64, rain: f64, icon: &'static str) -> DayForecast {
        DayForecast {
            date: date.into(),
            weekday: weekday.into(),
            symbol: String::new(),
            icon,
            color: (0, 0, 0),
            temp_min: min,
            temp_max: max,
            precip_mm: rain,
        }
    }

    /// Text before the icon, measured in characters (all single-width).
    fn before_icon(line: &Line) -> usize {
        let spans = &line.spans[..line.spans.len() - 1];
        spans.iter().map(|s| s.content.chars().count()).sum()
    }

    #[test]
    fn columns_line_up_whatever_the_values() {
        let rows = [
            day("Po", "2026-11-24", -12.4, -3.0, 12.5, "❄"),
            day("Út", "2026-11-05", 3.0, 27.6, 0.0, "☀"),
            day("St", "2026-09-30", -0.4, 4.0, 0.3, "🌧"),
        ];
        let widths: Vec<usize> = rows.iter().map(|d| before_icon(&day_row(d, false))).collect();
        assert!(widths.windows(2).all(|w| w[0] == w[1]), "{widths:?}");
    }

    #[test]
    fn values_read_cleanly() {
        assert_eq!(temp(-0.4), "0°");
        assert_eq!(temp(-3.0), "−3°");
        assert_eq!(temp(12.6), "13°");
        assert_eq!(date_label("2026-09-05"), "5.9.");
        let text: String = day_row(&day("St", "2026-09-30", -2.0, 4.0, 2.06, "🌧"), false)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(text, "  St  30.9.   −2° …   4°  2,1 mm  🌧");
    }
}
