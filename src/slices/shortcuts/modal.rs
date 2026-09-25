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

use super::usage::state::{Panel, UsageOverview};
use crate::shared::theme;
use document::documents;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, LineGauge, Paragraph},
    Frame,
};

/// Renders the overview across the whole given area with four side-by-side panels.
///
/// Takes `&mut` because it caches the document geometry (`rows`, `view_h`) back
/// into the state so paging and `End` are exact for each panel.
pub fn render_usage_panel(frame: &mut Frame, area: Rect, state: &mut UsageOverview) {
    theme::paint_backdrop(frame, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Min(5),    // panel area
            Constraint::Length(2), // footer
        ])
        .split(area);

    render_title(frame, rows[0], state);

    let progress = state
        .scan_progress()
        .map(|p| (p.done(), p.total(), p.ratio()));
    let panel_area = rows[1];

    match progress {
        Some((done, total, ratio)) => {
            // During scanning, show progress in the center of the panel area
            render_progress(frame, panel_area, done, total, ratio);
            // Reset all panel states
            for panel in Panel::ALL {
                let ps = state.panel_state_mut(panel);
                ps.rows = 0;
                ps.view_h = panel_area.height;
                ps.scroll = 0;
            }
        }
        None => {
            if let Some(stats) = state.stats.as_ref() {
                let docs = documents(stats);
                // Calculate panel widths - equal distribution with minimum width
                let panel_width = panel_area.width / Panel::COUNT as u16;
                let extra = panel_area.width % Panel::COUNT as u16;

                let focus = state.focus();
                let mut x = panel_area.x;
                for (i, panel) in Panel::ALL.iter().enumerate() {
                    let w = panel_width + if (i as u16) < extra { 1 } else { 0 };
                    let panel_rect = Rect {
                        x,
                        y: panel_area.y,
                        width: w,
                        height: panel_area.height,
                    };
                    x += w;

                    let doc = docs.get(*panel);
                    let ps = state.panel_state_mut(*panel);
                    let block = Block::bordered().border_type(BorderType::Rounded);
                    let inner = block.inner(panel_rect);
                    ps.view_h = inner.height;
                    ps.rows = doc.len() as u16;
                    ps.scroll = ps.scroll.min(ps.rows.saturating_sub(ps.view_h));

                    let is_focused = *panel == focus;
                    let block = Block::bordered()
                        .border_type(BorderType::Rounded)
                        .border_style(if is_focused {
                            Style::default().fg(theme::MODAL_ACCENT)
                        } else {
                            Style::default().fg(Color::Rgb(80, 80, 100))
                        })
                        .style(Style::default().bg(theme::MODAL_BG))
                        .title(Span::styled(
                            format!(" {} ", panel.title()),
                            Style::default().fg(theme::MODAL_ACCENT).bold(),
                        ));

                    let inner = block.inner(panel_rect);
                    frame.render_widget(block, panel_rect);
                    frame.render_widget(Paragraph::new(doc.clone()).scroll((ps.scroll, 0)), inner);
                }
            } else {
                // No data yet - show empty state centered
                let inner = theme::centered_percent(panel_area, 70, 40);
                for panel in Panel::ALL {
                    let ps = state.panel_state_mut(panel);
                    ps.rows = 0;
                    ps.view_h = panel_area.height;
                    ps.scroll = 0;
                }
                render_empty(frame, inner, state);
            }
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

    // The scroll position is for the currently focused panel.
    let ps = state.panel_state(state.focus());
    let position = if ps.rows > ps.view_h {
        format!(
            "{} {}–{} / {}  ",
            state.focus().title(),
            ps.scroll + 1,
            (ps.scroll + ps.view_h).min(ps.rows),
            ps.rows
        )
    } else {
        format!("{} {} řádků  ", state.focus().title(), ps.rows)
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(16), Constraint::Length(32)])
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
        Span::styled("  [←/→]", Style::default().fg(Color::Cyan).bold()),
        Span::styled(" panel  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[↑/↓]", Style::default().fg(Color::Cyan).bold()),
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
