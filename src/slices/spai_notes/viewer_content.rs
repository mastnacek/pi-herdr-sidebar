//! Formatter for the right-hand SPAI note viewer.
use super::note::{SpaiNoteItem, SpaiStatus};
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span},
};

pub fn format_viewer_content(item: &SpaiNoteItem) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(Line::raw(""));

    // Metadata header
    let status_color = match item.status {
        SpaiStatus::Done => Color::Rgb(55, 244, 153),
        SpaiStatus::Working => Color::Rgb(241, 252, 121),
        SpaiStatus::Waiting => Color::Rgb(189, 147, 249),
        SpaiStatus::Cancelled => Color::DarkGray,
        SpaiStatus::Idea => Color::Rgb(255, 121, 198),
        SpaiStatus::Note => Color::Rgb(139, 233, 253),
        _ => Color::White,
    };

    lines.push(Line::from(vec![
        Span::styled("  Stav:     ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} {}", item.status.glyph(), item.status.as_str()),
            Style::default().fg(status_color).bold(),
        ),
        Span::styled("    Typ: ", Style::default().fg(Color::DarkGray)),
        Span::styled(item.kind.as_str(), Style::default().fg(Color::White)),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  Vytvořeno:", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(" {}", item.timestamp),
            Style::default().fg(Color::Gray),
        ),
    ]));

    if let Some(proj) = &item.facets.project {
        lines.push(Line::from(vec![
            Span::styled("  Projekt:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(proj.clone(), Style::default().fg(Color::Cyan)),
        ]));
    }

    if let Some(p) = &item.facets.priority {
        lines.push(Line::from(vec![
            Span::styled("  Priorita: ", Style::default().fg(Color::DarkGray)),
            Span::styled(p.clone(), Style::default().fg(Color::Yellow)),
        ]));
    }

    if !item.tags.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  Štítky:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(item.tags.join(", "), Style::default().fg(Color::Magenta)),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("  Soubor:   ", Style::default().fg(Color::DarkGray)),
        Span::styled(item.file_name.clone(), Style::default().fg(Color::DarkGray)),
    ]));

    lines.push(Line::styled(
        "  ───────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    ));
    lines.push(Line::raw(""));

    // Body lines
    for line in item.body.lines() {
        if line.starts_with("# ") {
            lines.push(Line::styled(
                format!("  {}", line),
                Style::default().fg(Color::Green).bold(),
            ));
        } else if line.starts_with("## ") {
            lines.push(Line::styled(
                format!("  {}", line),
                Style::default().fg(Color::Cyan).bold(),
            ));
        } else if line.starts_with("### ") {
            lines.push(Line::styled(
                format!("  {}", line),
                Style::default().fg(Color::Yellow).bold(),
            ));
        } else if line.starts_with("- ") || line.starts_with("* ") {
            lines.push(Line::styled(
                format!("  {}", line),
                Style::default().fg(Color::White),
            ));
        } else {
            lines.push(Line::styled(
                format!("  {}", line),
                Style::default().fg(Color::White),
            ));
        }
    }

    lines
}
