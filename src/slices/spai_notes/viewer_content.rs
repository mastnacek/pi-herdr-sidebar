//! Formatter for the right-hand SPAI note viewer with 5D facets display.
use super::note::{SpaiNoteItem, SpaiStatus};
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span},
};

pub fn format_viewer_content(item: &SpaiNoteItem) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(Line::raw(""));

    // 1. Status & Kind
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

    // 2. 5D Facets Metadata Section
    let mut facet_spans = Vec::new();
    if let Some(area) = &item.facets.area {
        facet_spans.push(Span::styled(" Oblast: ", Style::default().fg(Color::DarkGray)));
        facet_spans.push(Span::styled(area.clone(), Style::default().fg(Color::Rgb(45, 213, 183)).bold()));
    }
    if let Some(effort) = &item.facets.effort {
        facet_spans.push(Span::styled("  Náročnost: ", Style::default().fg(Color::DarkGray)));
        facet_spans.push(Span::styled(effort.clone(), Style::default().fg(Color::Rgb(241, 252, 121))));
    }
    if let Some(urgency) = &item.facets.urgency {
        facet_spans.push(Span::styled("  Naléhavost: ", Style::default().fg(Color::DarkGray)));
        facet_spans.push(Span::styled(urgency.clone(), Style::default().fg(Color::Rgb(255, 121, 198))));
    }
    if let Some(who) = &item.facets.who {
        facet_spans.push(Span::styled("  Řešitel: ", Style::default().fg(Color::DarkGray)));
        facet_spans.push(Span::styled(who.clone(), Style::default().fg(Color::Rgb(189, 147, 249))));
    }
    if let Some(dl) = &item.facets.deadline {
        facet_spans.push(Span::styled("  Termín: ", Style::default().fg(Color::DarkGray)));
        facet_spans.push(Span::styled(dl.clone(), Style::default().fg(Color::Rgb(255, 83, 69)).bold()));
    }

    if !facet_spans.is_empty() {
        let mut f_line = vec![Span::styled("  5D Facety:", Style::default().fg(Color::DarkGray).bold())];
        f_line.extend(facet_spans);
        lines.push(Line::from(f_line));
    }

    lines.push(Line::styled(
        "  ───────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    ));
    lines.push(Line::raw(""));

    // 3. Body lines
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
