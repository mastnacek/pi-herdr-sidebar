//! Ctrl+D dedup popup (plan §3): anchored under the cursor line, non-modal.
//! Render only; interactions come from the key layer (↑/↓/Ctrl+O/A/U/Esc).
use super::super::state::SpaiNotesState;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
    Frame,
};

pub fn render_dedup_popup(frame: &mut Frame, buffer_area: Rect, state: &SpaiNotesState) {
    let dedup = &state.scratch.dedup;
    if !dedup.visible {
        return;
    }

    // Width: min(60, area−2). Height: lines(min:2) + 1 row of keys.
    let width = buffer_area.width.saturating_sub(2).min(60);
    let rows: u16 = if dedup.is_evaluating {
        1
    } else if dedup.matches.is_empty() {
        1
    } else {
        dedup.matches.len() as u16 + 1
    };
    let height: u16 = rows + 2;
    if width == 0 || buffer_area.height < height {
        return;
    }

    // Anchor: below the cursor line (accounting for filtered-out records).
    let Some(cursor_y) = super::line_render::cursor_visual_row(buffer_area, state) else {
        return;
    };
    let anchor_y = cursor_y + 1;

    let mut y = anchor_y;
    if y + height > buffer_area.y + buffer_area.height {
        // No room below → render above the anchor line.
        y = anchor_y.saturating_sub(height);
    }
    y = y.max(buffer_area.y);

    let area = Rect::new(buffer_area.x + 1, y, width, height);

    let block = Block::bordered()
        .title(Span::styled(
            " ⚡ duplicity (Ctrl+D) ",
            Style::default().fg(Color::Rgb(255, 184, 108)).bold(),
        ))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(255, 184, 108)));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    if dedup.is_evaluating {
        lines.push(Line::from(Span::styled(
            " ⏳ vyhodnocuji… (místní, bez sítě — embeddingy když jsou dostupné)",
            Style::default().fg(Color::DarkGray),
        )));
    } else if dedup.matches.is_empty() {
        lines.push(Line::from(Span::styled(
            " ✨ Žádné duplicity — záznam je unikátní",
            Style::default().fg(Color::Green),
        )));
    } else {
        for (i, m) in dedup.matches.iter().enumerate() {
            let sel = i == dedup.selected;
            let style = if sel {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Rgb(60, 60, 20))
                    .bold()
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let pct = (m.similarity * 100.0).round() as u32;
            lines.push(Line::from(vec![
                Span::styled(format!(" {:>2}% ", pct), style),
                Span::styled(
                    format!("{} ", m.symbol),
                    Style::default().fg(Color::Yellow).bold(),
                ),
                Span::styled(
                    format!("{}: {}", m.id, m.title),
                    style,
                ),
            ]));
        }
        lines.push(Line::from(vec![
            Span::styled(" [↑/↓] ", Style::default().fg(Color::Yellow).bold()),
            Span::styled("vybrat  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Ctrl+O] ", Style::default().fg(Color::Green).bold()),
            Span::styled("otevřít  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Ctrl+A] ", Style::default().fg(Color::Green).bold()),
            Span::styled("připojit  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Ctrl+U] ", Style::default().fg(Color::Green).bold()),
            Span::styled("stav  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Esc] ", Style::default().fg(Color::Cyan).bold()),
            Span::styled("zavřít", Style::default().fg(Color::DarkGray)),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().add_modifier(Modifier::empty())),
        inner,
    );
}