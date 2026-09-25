//! Rendering of the Shortcuts tab: the keybindings your Herdr actually has,
//! plus a detail panel for the selected chord.
use super::keys::ShortcutSource;
use super::model::ShortcutsState;
use crate::shared::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

const KEY_COL: usize = 20;

pub fn render_shortcuts_tab(frame: &mut Frame, area: Rect, state: &ShortcutsState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(6),    // binding list
            Constraint::Length(7), // detail
            Constraint::Length(2), // footer
        ])
        .split(area);

    render_header(frame, chunks[0], state);
    render_list(frame, chunks[1], state);
    render_detail(frame, chunks[2], state);
    render_footer(frame, chunks[3], state);
}

fn render_header(frame: &mut Frame, area: Rect, state: &ShortcutsState) {
    let user = state.user_binding_count();
    let source = state
        .config_path
        .as_deref()
        .map(shorten_path)
        .unwrap_or_else(|| "config.toml nenalezen".to_string());

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            format!(
                " ⌨  Zkratky · prefix {} · {} vazeb ({} z configu) ",
                state.prefix,
                state.entries.len(),
                user
            ),
            Style::default().fg(Color::Cyan).bold(),
        ));

    let body = Paragraph::new(Line::from(vec![
        Span::styled("  zdroj: ", Style::default().fg(Color::DarkGray)),
        Span::styled(source, Style::default().fg(Color::Gray)),
    ]))
    .block(block);

    frame.render_widget(body, area);
}

fn render_list(frame: &mut Frame, area: Rect, state: &ShortcutsState) {
    let view_h = area.height.saturating_sub(2).max(1) as usize;

    // Follow the selection without needing `&mut` during render.
    let mut scroll = state.scroll as usize;
    if state.selected < scroll {
        scroll = state.selected;
    } else if state.selected >= scroll + view_h {
        scroll = state.selected + 1 - view_h;
    }

    let mut lines: Vec<Line> = Vec::new();
    for (idx, entry) in state.entries.iter().enumerate().skip(scroll).take(view_h) {
        let selected = idx == state.selected;
        let (badge, badge_color) = match entry.source {
            ShortcutSource::User => ("config", Color::Rgb(55, 244, 153)),
            ShortcutSource::Default => ("výchozí", Color::DarkGray),
            ShortcutSource::Suggested => ("návrh", Color::Rgb(255, 121, 198)),
        };

        let mut spans = vec![
            Span::styled(
                if selected { "▶ " } else { "  " },
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!("{:<width$}", truncate(&entry.key, KEY_COL), width = KEY_COL),
                Style::default()
                    .fg(if selected {
                        Color::Yellow
                    } else {
                        theme::MODAL_ACCENT
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<9}", truncate(badge, 9)),
                Style::default().fg(badge_color),
            ),
            Span::styled(
                entry.description.clone(),
                Style::default().fg(if selected { Color::White } else { Color::Gray }),
            ),
        ];

        if !entry.command.is_empty() {
            spans.push(Span::styled(
                format!("  {}", entry.command),
                Style::default().fg(Color::DarkGray),
            ));
        }

        let mut line = Line::from(spans);
        if selected {
            line = line.style(Style::default().bg(Color::Rgb(30, 35, 55)));
        }
        lines.push(line);
    }

    if lines.is_empty() {
        lines.push(Line::styled(
            "  Žádné zkratky nenalezeny.",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let list = Paragraph::new(lines).block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                format!(" Vazby ({}/{}) ", state.selected + 1, state.entries.len()),
                Style::default().fg(Color::Gray),
            )),
    );
    frame.render_widget(list, area);
}

fn render_detail(frame: &mut Frame, area: Rect, state: &ShortcutsState) {
    let lines = match state.selected_entry() {
        Some(entry) => {
            let (kind_label, source_label) = match entry.source {
                ShortcutSource::User => ("spouští plugin / shell", "config.toml"),
                ShortcutSource::Default => ("výchozí Herdr vazba", "Herdr default"),
                ShortcutSource::Suggested => {
                    ("doporučená vazba — přidejte do config.toml", "návrh")
                }
            };
            vec![
                Line::from(vec![
                    Span::styled("  Klávesa:  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(entry.key.clone(), Style::default().fg(Color::Yellow).bold()),
                ]),
                Line::from(vec![
                    Span::styled("  Akce:     ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        if entry.command.is_empty() {
                            entry.description.clone()
                        } else {
                            entry.command.clone()
                        },
                        Style::default().fg(theme::MODAL_ACCENT),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  Typ:      ", Style::default().fg(Color::DarkGray)),
                    Span::styled(entry.kind.clone(), Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("   ({kind_label})"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  Zdroj:    ", Style::default().fg(Color::DarkGray)),
                    Span::styled(source_label, Style::default().fg(Color::Gray)),
                ]),
            ]
        }
        None => vec![Line::styled(
            "  Vyberte zkratku.",
            Style::default().fg(Color::DarkGray),
        )],
    };

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(" Detail ", Style::default().fg(Color::Gray)));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_footer(frame: &mut Frame, area: Rect, state: &ShortcutsState) {
    let status = state
        .status
        .clone()
        .unwrap_or_else(|| "m = statistiky použití pluginů a skillů".to_string());

    let lines = vec![
        Line::from(vec![
            Span::styled("  [↑/↓]", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" výběr  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[PgUp/PgDn]", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" posun  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[m]", Style::default().fg(Color::Yellow).bold()),
            Span::styled(" statistiky  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[r]", Style::default().fg(Color::Cyan).bold()),
            Span::styled(" znovu načíst config", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                status,
                Style::default().fg(if state.status.is_some() {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

/// Shortens a long path for the header, keeping the tail visible.
pub(crate) fn shorten_path(path: &str) -> String {
    const MAX: usize = 52;
    if path.chars().count() <= MAX {
        return path.to_string();
    }
    let tail: String = path
        .chars()
        .rev()
        .take(MAX - 3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("…{tail}")
}

/// Truncates to `max` chars, appending `…` when cut.
pub(crate) fn truncate(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate("příliš žluťoučký", 5), "příl…");
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("anything", 0), "");
    }

    #[test]
    fn shorten_path_keeps_the_tail() {
        let long = "C:/Users/jaroslav/AppData/Roaming/herdr/deep/nested/config.toml";
        assert!(long.chars().count() > 52, "fixture must exceed the limit");
        let short = shorten_path(long);
        assert!(short.starts_with('…'));
        assert!(short.ends_with("config.toml"));
        assert!(short.chars().count() <= 52);
        assert_eq!(
            shorten_path("a/b.toml"),
            "a/b.toml",
            "short paths are untouched"
        );
    }
}
