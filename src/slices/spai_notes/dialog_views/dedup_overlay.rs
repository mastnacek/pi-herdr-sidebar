//! Overlay panel with on-demand duplicate-check results (Ctrl+D).
//!
//! Rendered above whichever dialog is active (creation or editor); `Esc`
//! closes it, `Ctrl+O/A/U` act on the matches (creation dialog).
use crate::slices::spai_notes::state::SpaiNotesState;
use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Wrap},
    Frame,
};

/// Draws the panel when `state.dedup.visible`. Anchored to the lower part of
/// `area` so it never covers the input line at the top.
pub fn render_dedup_overlay(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    if !state.dedup.visible {
        return;
    }

    let eval_lines = 1usize;
    let matches_lines = state.dedup.matches.len().max(1);
    let height = (matches_lines + eval_lines + 2).clamp(3, area.height.saturating_sub(6) as usize) as u16;
    let width = area.width.saturating_sub(8).max(20);

    let panel = Rect {
        x: area.x + 4,
        y: area.y + area.height.saturating_sub(height + 4),
        width,
        height,
    };

    frame.render_widget(Clear, panel);

    let in_creation = state.creation_dialog.active;
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(255, 184, 108)));

    let mut lines: Vec<Line> = Vec::new();

    if state.dedup.is_evaluating {
        block = block.title(Span::styled(
            " ⠋ Kontrola duplicit… ",
            Style::default().fg(Color::Yellow).bold(),
        ));
        lines.push(Line::from(Span::styled(
            " Vektorizuji a porovnávám přes OpenRouter…",
            Style::default().fg(Color::Yellow),
        )));
    } else if state.dedup.matches.is_empty() {
        block = block.title(Span::styled(
            " ✓ Žádné duplicity ",
            Style::default().fg(Color::Rgb(55, 244, 153)).bold(),
        ));
        lines.push(Line::from(Span::styled(
            " Podobný záznam nebyl nalezen (text + vektory).",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        block = block.title(Span::styled(
            " ⚠ Podobné záznamy ",
            Style::default().fg(Color::Rgb(255, 184, 108)).bold(),
        ));
        for m in &state.dedup.matches {
            let pct = (m.similarity * 100.0).round() as u32;
            let color = if pct >= 70 {
                Color::Rgb(255, 83, 69)
            } else {
                Color::Rgb(255, 184, 108)
            };
            let tag = if m.is_vector_match { "[Vektor] " } else { "" };
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" [{:>2}%] {}{}", pct, tag, m.symbol),
                    Style::default().fg(color).bold(),
                ),
                Span::styled(
                    format!(" {}: {}", m.id, m.title),
                    Style::default().fg(Color::White),
                ),
            ]));
        }
    }

    // Action hints — only the creation dialog offers O/A/U.
    if in_creation && !state.dedup.is_evaluating && !state.dedup.matches.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(" [Ctrl+O] ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
            Span::styled("otevřít ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Ctrl+A] ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
            Span::styled("připojit ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Ctrl+U] ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
            Span::styled("stav ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Esc] ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
            Span::styled("zpět", Style::default().fg(Color::DarkGray)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled(" [Esc] ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
            Span::styled("zpět", Style::default().fg(Color::DarkGray)),
        ]));
    }

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, panel);
}
