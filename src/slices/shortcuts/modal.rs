//! The plugin-usage panel: what your plugins and skills actually get used for,
//! read from pi's own session logs.
//!
//! [`render_usage_panel`] paints the **whole** given area with the shared modal
//! backdrop and renders one scrollable document inside it. It is rendered by the
//! standalone overview window ([`super::overview`], `pi_sidebar usage`), which
//! owns the entire pane — the Shortcuts tab never draws this, so the overview
//! carries no sidebar chrome.
//!
//! The document lists *every* plugin, skill, command and tool — nothing is
//! truncated to a "… and N more" row. Rows are never wrapped (long names are
//! clipped instead), which keeps the row count independent of the terminal width
//! and therefore makes scrolling and paging exact.
mod document;

use super::usage::state::UsageOverview;
use crate::shared::theme;
pub use document::document;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, LineGauge, Paragraph},
    Frame,
};

/// Renders the overview across the whole given area.
///
/// Takes `&mut` because it caches the document geometry (`rows`, `view_h`) back
/// into the state so paging and `End` are exact.
pub fn render_usage_panel(frame: &mut Frame, area: Rect, state: &mut UsageOverview) {
    theme::paint_backdrop(frame, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Min(5),    // scrolling document
            Constraint::Length(2), // footer
        ])
        .split(area);

    render_title(frame, rows[0], state);

    // Read what we need up front so the state is free to be mutated below.
    let doc = state.stats.as_ref().map(document);
    let progress = state
        .scan_progress()
        .map(|p| (p.done(), p.total(), p.ratio()));
    let body_h = rows[1].height;

    state.view_h = body_h;
    match (progress, doc) {
        (Some((done, total, ratio)), _) => {
            state.rows = 0;
            state.scroll = 0;
            render_progress(frame, rows[1], done, total, ratio);
        }
        (None, Some(doc)) => {
            state.rows = doc.len() as u16;
            state.scroll = state.scroll.min(state.max_scroll());
            frame.render_widget(Paragraph::new(doc).scroll((state.scroll, 0)), rows[1]);
        }
        (None, None) => {
            state.rows = 0;
            state.scroll = 0;
            render_empty(frame, rows[1], state);
        }
    }

    render_footer(frame, rows[2], state);
}

fn render_title(frame: &mut Frame, area: Rect, state: &UsageOverview) {
    let scanned = state
        .stats
        .as_ref()
        .map(|s| format!(" · {} souborů", format_count(s.files as u64)))
        .unwrap_or_default();

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::MODAL_ACCENT))
        .style(Style::default().bg(theme::MODAL_BG))
        .title(Span::styled(
            " 📊  Využití pluginů a skillů · pi session logy ",
            Style::default().fg(theme::MODAL_ACCENT).bold(),
        ));

    let line = Line::from(vec![
        Span::styled("  zdroj: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            super::usage::sessions_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "~/.pi/agent/sessions".to_string()),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(scanned, Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn render_progress(frame: &mut Frame, area: Rect, done: usize, total: usize, ratio: f64) {
    let inner = theme::centered_percent(area, 70, 40);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // heading
            Constraint::Length(1),
            Constraint::Length(1), // gauge
            Constraint::Length(1),
            Constraint::Length(1), // hint
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Line::styled(
            "  Prohledávám session logy…",
            Style::default().fg(Color::White).bold(),
        )),
        rows[0],
    );

    let gauge = LineGauge::default()
        .filled_style(Style::default().fg(theme::MODAL_ACCENT).bold())
        .unfilled_style(Style::default().fg(Color::Rgb(55, 55, 70)))
        .label(Span::styled(
            format!(
                " {} / {} souborů ",
                format_count(done as u64),
                format_count(total as u64)
            ),
            Style::default().fg(Color::White),
        ))
        .ratio(ratio.clamp(0.0, 1.0));
    frame.render_widget(gauge, rows[2]);

    frame.render_widget(
        Paragraph::new(Line::styled(
            "  První sken prochází celý adresář; výsledek se pak ukládá do cache.",
            Style::default().fg(Color::DarkGray),
        )),
        rows[4],
    );
}

fn render_empty(frame: &mut Frame, area: Rect, state: &UsageOverview) {
    let mut lines = vec![
        Line::raw(""),
        Line::styled(
            "  Zatím žádná data.",
            Style::default().fg(Color::White).bold(),
        ),
        Line::raw(""),
        Line::styled(
            "  Stiskněte [r] pro spuštění skenu session logů.",
            Style::default().fg(Color::DarkGray),
        ),
    ];
    if let Some(status) = state.status.clone().filter(|s| !s.is_empty()) {
        lines.push(Line::styled(
            format!("  {status}"),
            Style::default().fg(Color::Yellow),
        ));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_footer(frame: &mut Frame, area: Rect, state: &UsageOverview) {
    let totals = state
        .stats
        .as_ref()
        .map(|s| {
            // Plugin and skill counts are already in the section headers, so the
            // footer stays short enough not to clip on a narrow pane.
            format!(
                "  {} souborů · {} zpráv · {} volání · {} ms",
                format_count(s.files as u64),
                format_count(s.messages),
                format_count(s.tool_calls),
                format_count(s.elapsed_ms)
            )
        })
        .unwrap_or_else(|| "  žádná data".to_string());

    // The scroll position is the one thing needed *while* scrolling, so it gets
    // its own right-aligned column: on a narrow pane the long totals string may
    // clip, but the position never does.
    let position = if state.rows > state.view_h {
        format!(
            "řádky {}–{} / {}  ",
            state.scroll + 1,
            (state.scroll + state.view_h).min(state.rows),
            state.rows
        )
    } else {
        format!("{} řádků  ", state.rows)
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(16), Constraint::Length(24)])
        .split(rows[0]);

    frame.render_widget(
        Paragraph::new(Line::styled(totals, Style::default().fg(Color::Gray))),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(Line::styled(
            position,
            Style::default().fg(theme::MODAL_ACCENT),
        ))
        .alignment(Alignment::Right),
        cols[1],
    );

    let keys = Line::from(vec![
        Span::styled("  [↑/↓]", Style::default().fg(Color::Cyan).bold()),
        Span::styled(" posun  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[PgUp/PgDn]", Style::default().fg(Color::Cyan).bold()),
        Span::styled(" stránka  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Home/End]", Style::default().fg(Color::Cyan).bold()),
        Span::styled(" začátek/konec  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[r]", Style::default().fg(Color::Cyan).bold()),
        Span::styled(" přeskenovat  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Esc]", Style::default().fg(Color::Yellow).bold()),
        Span::styled(" zavřít", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(keys), rows[1]);
}

/// Groups thousands with a thin space, Czech style (`70 627`).
pub(crate) fn format_count(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    // `char_indices` keeps the lint happy and, for ASCII digits, the byte offset
    // is the same as the character position.
    for (i, c) in digits.char_indices() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests;
