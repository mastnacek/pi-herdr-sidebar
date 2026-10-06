//! `@` project autocomplete popup: anchored under the cursor line, rendered
//! while a mention token is open (Edit mode). Interactions come from the key
//! layer (↑/↓/Enter/Tab/Esc).
use super::super::state::SpaiNotesState;
use super::mention::mention_matches;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
    Frame,
};

pub fn render_mention_popup(frame: &mut Frame, buffer_area: Rect, state: &SpaiNotesState) {
    if state.scratch.mention.is_none() || state.scratch.mode != super::state::ScratchMode::Edit {
        return;
    }
    let matches = mention_matches(&state.scratch, &state.projects);
    let selected = state.scratch.mention.as_ref().map(|m| m.selected).unwrap_or(0);

    let width = buffer_area.width.saturating_sub(2).min(52);
    let rows = matches.len() as u16 + 1; // + the key-hint row
    let height = rows + 2;
    if width == 0 || buffer_area.height < height {
        return;
    }

    // Anchor below the cursor line (same rule as the dedup popup).
    let (scroll, _) = super::view::visible_window(
        state.scratch.lines.len(),
        state.scratch.cursor_line,
        buffer_area.height as usize,
    );
    let rel = state.scratch.cursor_line.saturating_sub(scroll);
    let mut y = buffer_area.y + 1 + rel as u16 + 1;
    if y + height > buffer_area.y + buffer_area.height {
        y = buffer_area.y + 1 + rel as u16 + 1 - height; // above the line
    }
    y = y.max(buffer_area.y);

    let area = Rect::new(buffer_area.x + 1, y, width, height);

    let block = Block::bordered()
        .title(Span::styled(
            " @ projekty ",
            Style::default().fg(Color::Cyan).bold(),
        ))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    if matches.is_empty() {
        lines.push(Line::from(Span::styled(
            " žádný projekt neodpovídá",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, p) in matches.iter().enumerate() {
            let sel = i == selected;
            let style = if sel {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Rgb(20, 50, 60))
                    .bold()
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let path = p.path.to_string_lossy();
            lines.push(Line::from(vec![
                Span::styled(format!(" @{} ", p.name), style),
                Span::styled(path.to_string(), Style::default().fg(Color::DarkGray)),
            ]));
        }
    }
    lines.push(Line::from(vec![
        Span::styled(" [↑/↓] ", Style::default().fg(Color::Yellow).bold()),
        Span::styled("vybrat  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Enter] ", Style::default().fg(Color::Green).bold()),
        Span::styled("doplnit  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Esc] ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("skrýt", Style::default().fg(Color::DarkGray)),
    ]));

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().add_modifier(Modifier::empty()))
            .wrap(ratatui::widgets::Wrap { trim: false }),
        inner,
    );
}
