//! Builds the overview document: one `Line` per row, no truncation.
//!
//! Kept apart from the rendering in the parent module so each stays small.
//! Rows never wrap, which keeps the row count independent of the terminal
//! width and makes scrolling exact.
use super::format_count;
use crate::shared::theme;
use crate::slices::shortcuts::usage::model::{plugin_of, Counted, PluginUsage};
use crate::slices::shortcuts::usage::UsageStats;
use crate::slices::shortcuts::view::truncate;
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span},
};

/// Width of the proportional bars.
const BAR: usize = 12;
/// Width of the item-name column.
const NAME_W: usize = 22;

/// Builds the whole document, one `Line` per row.
pub fn document(stats: &UsageStats) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let plugins = stats.ranked_plugins();
    section(
        &mut lines,
        format!(
            "Pluginy ({}) · {} volání",
            plugins.len(),
            format_count(stats.plugin_calls())
        ),
    );
    if plugins.is_empty() {
        empty_row(&mut lines);
    }
    let max = plugins.first().map(|p| p.calls).unwrap_or(0);
    for plugin in &plugins {
        lines.push(bar_row(
            &plugin.plugin,
            plugin.calls,
            max,
            Color::Rgb(55, 244, 153),
            Some(tool_hint(plugin)),
        ));
    }

    section(
        &mut lines,
        format!("Skilly ({}) · čtení SKILL.md", stats.skills.len()),
    );
    if stats.skills.is_empty() {
        empty_row(&mut lines);
    }
    let max = stats.skills.first().map(|s| s.count).unwrap_or(0);
    for skill in &stats.skills {
        lines.push(bar_row(
            &skill.name,
            skill.count,
            max,
            Color::Rgb(255, 121, 198),
            None,
        ));
    }

    section(&mut lines, format!("Příkazy ({})", stats.commands.len()));
    if stats.commands.is_empty() {
        empty_row(&mut lines);
    }
    let max = stats.commands.first().map(|c| c.count).unwrap_or(0);
    for command in &stats.commands {
        lines.push(bar_row(
            &command.name,
            command.count,
            max,
            Color::Rgb(189, 147, 249),
            None,
        ));
    }

    section(
        &mut lines,
        format!(
            "Nástroje ({}) · vestavěné {}",
            stats.tools.len(),
            format_count(stats.builtin_calls)
        ),
    );
    if stats.tools.is_empty() {
        empty_row(&mut lines);
    }
    for tool in &stats.tools {
        lines.push(tool_row(tool));
    }

    lines
}

/// Inserts a blank separator (except at the very top) and a section header.
fn section(lines: &mut Vec<Line<'static>>, title: String) {
    if !lines.is_empty() {
        lines.push(Line::raw(""));
    }
    lines.push(Line::from(vec![
        Span::styled("▌ ", Style::default().fg(theme::MODAL_ACCENT)),
        Span::styled(title, Style::default().fg(Color::White).bold()),
    ]));
}

fn empty_row(lines: &mut Vec<Line<'static>>) {
    lines.push(Line::styled(
        "    (žádná data)",
        Style::default().fg(Color::DarkGray),
    ));
}

/// `name · bar · count · optional hint`
fn bar_row(name: &str, count: u64, max: u64, color: Color, hint: Option<String>) -> Line<'static> {
    let mut spans = vec![
        Span::styled(
            format!("  {:<NAME_W$}", truncate(name, NAME_W)),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(
            theme::usage_bar(count, max, BAR),
            Style::default().fg(color),
        ),
        Span::styled(
            format!(" {:>7}", format_count(count)),
            Style::default().fg(Color::White).bold(),
        ),
    ];
    if let Some(hint) = hint.filter(|h| !h.is_empty()) {
        spans.push(Span::styled(
            format!("  {hint}"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    Line::from(spans)
}

fn tool_row(tool: &Counted) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {:<NAME_W$}", truncate(&tool.name, NAME_W)),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(
            format!("{:>8}  ", format_count(tool.count)),
            Style::default().fg(Color::White).bold(),
        ),
        Span::styled(
            plugin_of(&tool.name).to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    ])
}

/// Up to three most used tools of a plugin, shown inline on its row.
fn tool_hint(plugin: &PluginUsage) -> String {
    plugin
        .tools
        .iter()
        .take(3)
        .map(|(name, count)| format!("{name} {}", format_count(*count)))
        .collect::<Vec<_>>()
        .join(" · ")
}
