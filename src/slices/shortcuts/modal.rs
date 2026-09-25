//! The plugin-usage panel: what your plugins and skills actually get used for,
//! read from pi's own session logs.
//!
//! [`render_usage_panel`] paints the **whole** given area with the shared modal
//! backdrop and lays the sections out inside it. It is rendered by the standalone
//! overview window ([`super::overview`], `pi_sidebar usage`), which owns the
//! entire pane — the Shortcuts tab never draws this, so the overview carries no
//! sidebar chrome.
use super::usage::state::UsageOverview;
use super::usage::UsageStats;
use crate::shared::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, LineGauge, Paragraph},
    Frame,
};

const ROW_BAR: usize = 10;
const MAX_PLUGINS: usize = 14;
const MAX_SKILLS: usize = 10;
const MAX_COMMANDS: usize = 8;

/// Renders the overview across the whole given area.
///
/// This is the standalone window (see [`super::overview`]), so there is no
/// centred dialog rect: the backdrop covers the entire pane and the sections sit
/// inside it, which is what makes it read as a dedicated modal window.
pub fn render_usage_panel(frame: &mut Frame, area: Rect, state: &UsageOverview) {
    theme::paint_backdrop(frame, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Min(10),   // columns
            Constraint::Length(5), // top tools
            Constraint::Length(2), // footer
        ])
        .split(area);

    render_title(frame, rows[0], state);

    if let Some(progress) = state.scan_progress() {
        render_progress(
            frame,
            rows[1],
            progress.done(),
            progress.total(),
            progress.ratio(),
        );
    } else if let Some(stats) = &state.stats {
        render_columns(frame, rows[1], stats);
        render_tools(frame, rows[2], stats);
    } else {
        render_empty(frame, rows[1], state);
    }

    render_footer(frame, rows[3], state);
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
            crate::slices::shortcuts::usage::sessions_dir()
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
    let lines = vec![
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
    let hint = state.status.clone().unwrap_or_default();
    let mut lines = lines;
    if !hint.is_empty() {
        lines.push(Line::styled(
            format!("  {hint}"),
            Style::default().fg(Color::Yellow),
        ));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_columns(frame: &mut Frame, area: Rect, stats: &UsageStats) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(38),
            Constraint::Percentage(32),
            Constraint::Percentage(30),
        ])
        .split(area);

    let plugins = stats.ranked_plugins();
    render_bars(
        frame,
        cols[0],
        &format!(" Pluginy · {} volání ", format_count(stats.plugin_calls())),
        plugins
            .iter()
            .take(MAX_PLUGINS)
            .map(|p| (p.plugin.clone(), p.calls))
            .collect(),
        plugins.first().map(|p| p.calls).unwrap_or(0),
        plugins.len().saturating_sub(MAX_PLUGINS),
        Color::Rgb(55, 244, 153),
    );

    let skill_max = stats.skills.first().map(|s| s.count).unwrap_or(0);
    render_bars(
        frame,
        cols[1],
        " Skilly · čtení SKILL.md ",
        stats
            .skills
            .iter()
            .take(MAX_SKILLS)
            .map(|s| (s.name.clone(), s.count))
            .collect(),
        skill_max,
        stats.skills.len().saturating_sub(MAX_SKILLS),
        Color::Rgb(255, 121, 198),
    );

    let command_max = stats.commands.first().map(|c| c.count).unwrap_or(0);
    render_bars(
        frame,
        cols[2],
        " Příkazy ",
        stats
            .commands
            .iter()
            .take(MAX_COMMANDS)
            .map(|c| (c.name.clone(), c.count))
            .collect(),
        command_max,
        stats.commands.len().saturating_sub(MAX_COMMANDS),
        Color::Rgb(189, 147, 249),
    );
}

fn render_bars(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    rows: Vec<(String, u64)>,
    max: u64,
    hidden: usize,
    color: Color,
) {
    let name_w = area.width.saturating_sub(ROW_BAR as u16 + 10).max(10) as usize;

    let mut lines: Vec<Line> = Vec::new();
    for (i, (name, count)) in rows.iter().enumerate() {
        let accent = if i == 0 { color } else { Color::Gray };
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:<name_w$}", super::view::truncate(name, name_w)),
                Style::default().fg(accent),
            ),
            Span::styled(
                theme::usage_bar(*count, max, ROW_BAR),
                Style::default().fg(color),
            ),
            Span::styled(
                format!(" {}", format_count(*count)),
                Style::default().fg(Color::White).bold(),
            ),
        ]));
    }
    if hidden > 0 {
        lines.push(Line::styled(
            format!("  … +{hidden} dalších"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    if lines.is_empty() {
        lines.push(Line::styled(
            "  (žádná data)",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(70, 70, 90)))
        .style(Style::default().bg(theme::MODAL_BG))
        .title(Span::styled(
            title.to_string(),
            Style::default().fg(theme::MODAL_ACCENT).bold(),
        ));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_tools(frame: &mut Frame, area: Rect, stats: &UsageStats) {
    let mut lines: Vec<Line> = Vec::new();
    for chunk in stats.tools.chunks(4).take(2) {
        let spans: Vec<Span> = chunk
            .iter()
            .flat_map(|tool| {
                vec![
                    Span::styled(
                        format!("  {}", super::view::truncate(&tool.name, 30)),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(
                        format!(" {}", format_count(tool.count)),
                        Style::default().fg(Color::White).bold(),
                    ),
                ]
            })
            .collect();
        lines.push(Line::from(spans));
    }
    if lines.is_empty() {
        lines.push(Line::styled(
            "  (žádná data)",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let builtins = stats
        .builtin_row()
        .map(|b| format!(" · vestavěné nástroje {} ", format_count(b.calls)))
        .unwrap_or_default();

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(70, 70, 90)))
        .style(Style::default().bg(theme::MODAL_BG))
        .title(Span::styled(
            format!(" Nejčastější nástroje{builtins}"),
            Style::default().fg(theme::MODAL_ACCENT),
        ));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_footer(frame: &mut Frame, area: Rect, state: &UsageOverview) {
    let totals = state
        .stats
        .as_ref()
        .map(|s| {
            format!(
                "{} souborů · {} zpráv · {} tool callů · sken {} ms",
                format_count(s.files as u64),
                format_count(s.messages),
                format_count(s.tool_calls),
                format_count(s.elapsed_ms)
            )
        })
        .unwrap_or_else(|| "žádná data".to_string());

    let lines = vec![
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(totals, Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled("  [r]", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" přeskenovat  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Esc]", Style::default().fg(Color::Yellow).bold()),
            Span::styled(" zavřít okno", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines), area);
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
