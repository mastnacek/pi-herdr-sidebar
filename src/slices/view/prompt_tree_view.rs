//! System-prompt tree for the Status face.
//!
//! Renders the replayed prompt sections in pi's documented build order — the
//! frame is always complete, so an absent `APPEND_SYSTEM.md` addendum is as
//! visible as a present one. Sources (`AGENTS.md` paths, the matching
//! `APPEND_SYSTEM.md` / `SYSTEM.md` file) hang below their section.

use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};

use crate::slices::telemetry::prompt_tree::{
    PromptSection, PromptSource, SourceState, SECTION_ORDER,
};
use crate::slices::telemetry::LiveTelemetry;

/// Human hint per documented section, in the same order as [`SECTION_ORDER`].
const SECTION_HINT: [(&str, &str); 8] = [
    ("preamble", "základní instrukce"),
    ("tools", "nástroje"),
    ("rules", "pravidla"),
    ("docs", "dokumentace pi"),
    ("addendum", "APPEND_SYSTEM.md"),
    ("project_context", "AGENTS.md / CLAUDE.md"),
    ("skills", "skilly"),
    ("cwd", "pracovní adresář"),
];

fn hint_for(name: &str) -> &str {
    SECTION_HINT
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, hint)| *hint)
        .unwrap_or("")
}

/// Build the Status-face lines for the replayed system prompt.
pub(super) fn prompt_tree_lines(t: &LiveTelemetry) -> Vec<Line<'static>> {
    let Some(tree) = t.prompt.as_ref() else {
        return vec![
            Line::from(vec![
                Span::styled(
                    "🧠 Systémový prompt: ",
                    Style::default().fg(Color::Cyan).bold(),
                ),
                Span::styled(
                    "v session logu zatím není systémová zpráva",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::raw(""),
        ];
    };

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            "🧠 Systémový prompt ",
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::styled(
            format!(
                "(pi replay · {} sekcí · {}{})",
                tree.sections.len(),
                fmt_chars(tree.total_chars),
                loaded_suffix(&tree.loaded_at)
            ),
            Style::default().fg(Color::DarkGray),
        ),
        if tree.patches > 0 {
            Span::styled(
                format!("  ⟳ {} pozdějších patchů", tree.patches),
                Style::default().fg(Color::Yellow),
            )
        } else {
            Span::raw("")
        },
    ]));

    let custom: Vec<&PromptSection> = tree.sections.iter().filter(|s| s.custom).collect();
    let total = SECTION_ORDER.len();
    for (index, name) in SECTION_ORDER.iter().enumerate() {
        let glyph = if index + 1 == total && custom.is_empty() {
            "└─"
        } else {
            "├─"
        };
        let section = tree.sections.iter().find(|s| s.name == *name);
        lines.push(section_line(glyph, index + 1, name, section));
        if let Some(section) = section {
            lines.extend(child_lines(glyph, section, tree));
        }
    }

    for (offset, section) in custom.iter().enumerate() {
        let glyph = if offset + 1 == custom.len() {
            "└─"
        } else {
            "├─"
        };
        lines.push(section_line(glyph, 0, &section.name, Some(section)));
    }

    lines.push(Line::raw(""));
    lines
}

fn section_line(
    glyph: &str,
    order: usize,
    name: &str,
    section: Option<&PromptSection>,
) -> Line<'static> {
    let color = section_color(name);
    let mut spans = vec![
        Span::styled(format!("{glyph} "), Style::default().fg(Color::DarkGray)),
        Span::styled(
            if order == 0 {
                format!("{:<22}", name)
            } else {
                format!("{order} {:<20}", name)
            },
            Style::default().fg(color).bold(),
        ),
    ];

    match section {
        None => spans.push(Span::styled(
            "— nepřítomno".to_string(),
            Style::default().fg(Color::DarkGray),
        )),
        Some(section) if section.removed => spans.push(Span::styled(
            format!("· odstraněno pozdějším patchem ({})", hint_for(name)),
            Style::default().fg(Color::Red),
        )),
        Some(section) => {
            spans.push(Span::styled(
                fmt_chars(section.chars),
                Style::default().fg(Color::Gray),
            ));
            if section.patches > 0 {
                spans.push(Span::styled(
                    format!(" ⟳{}", section.patches),
                    Style::default().fg(Color::Yellow),
                ));
            }
            if section.custom {
                spans.push(Span::styled(" [ext]", Style::default().fg(Color::DarkGray)));
            }
            if !section.preview.is_empty() && name != "project_context" {
                spans.push(Span::styled(
                    format!("  {}", truncate(&section.preview, 44)),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }
    }

    Line::from(spans)
}

/// Source rows hanging under a section that points at files.
fn child_lines(
    glyph: &str,
    section: &PromptSection,
    tree: &crate::slices::telemetry::prompt_tree::PromptTree,
) -> Vec<Line<'static>> {
    let indent = if glyph == "└─" {
        "     "
    } else {
        "│    "
    };
    let mut lines = Vec::new();

    match section.name.as_str() {
        "addendum" => match tree.append_system.as_ref() {
            Some((PromptSource::File(path), state)) => lines.push(child_with_state(
                indent,
                format!("← {}", shorten(path)),
                Color::Magenta,
                *state,
            )),
            Some((PromptSource::Inline, _)) => lines.push(child(
                indent,
                "← inline (--append-system-prompt / složené zdroje)".to_string(),
                Color::Magenta,
            )),
            None => {}
        },
        "preamble" => match tree.system_override.as_ref() {
            Some((PromptSource::File(path), state)) => lines.push(child_with_state(
                indent,
                format!("⚠ nahrazeno: {}", shorten(path)),
                Color::Yellow,
                *state,
            )),
            Some((PromptSource::Inline, _)) => lines.push(child(
                indent,
                "⚠ nahrazeno: --system-prompt / inline".to_string(),
                Color::Yellow,
            )),
            None => {}
        },
        "project_context" => {
            for (offset, file) in tree.context_files.iter().enumerate() {
                let branch = if offset + 1 == tree.context_files.len() {
                    "└─"
                } else {
                    "├─"
                };
                lines.push(child_with_state(
                    indent,
                    format!(
                        "{branch} {} ({})",
                        shorten(&file.path),
                        fmt_chars(file.chars)
                    ),
                    Color::Yellow,
                    file.state,
                ));
            }
        }
        "tools" => {
            lines.push(child(indent, tool_summary(tree), Color::DarkGray));
        }
        _ => {}
    }

    lines
}

/// `HH:MM:SS` of the newest system message — the moment the effective prompt
/// was assembled.
fn loaded_suffix(loaded_at: &str) -> String {
    let time = loaded_at
        .split('T')
        .nth(1)
        .and_then(|rest| rest.split('.').next())
        .filter(|t| !t.is_empty());
    match time {
        Some(time) => format!(" · načteno {time}"),
        None => String::new(),
    }
}

/// A loaded source row. Only files that were actually loaded get a row — an
/// unused candidate on disk is silence, not noise. Drift is the one thing
/// worth flagging: pi caches resources at session start and only re-reads them
/// on `/reload`.
fn child_with_state(
    indent: &str,
    text: String,
    color: Color,
    state: SourceState,
) -> Line<'static> {
    let mut line = child(indent, text, color);
    match state {
        SourceState::Ok => {}
        SourceState::Modified => line.spans.push(Span::styled(
            "  ⚠ změněno na disku — /reload",
            Style::default().fg(Color::Red).bold(),
        )),
        SourceState::Missing => line.spans.push(Span::styled(
            "  ⚠ soubor zmizel",
            Style::default().fg(Color::Red).bold(),
        )),
    }
    line
}

fn tool_summary(tree: &crate::slices::telemetry::prompt_tree::PromptTree) -> String {
    if tree.tools.is_empty() {
        return "· bez deklarovaných nástrojů".to_string();
    }
    let shown: Vec<&str> = tree.tools.iter().take(5).map(|s| s.as_str()).collect();
    let rest = tree.tools.len().saturating_sub(shown.len());
    let suffix = if rest > 0 {
        format!(" +{rest}")
    } else {
        String::new()
    };
    let mut summary = format!(
        "· {} nástrojů: {}{}",
        tree.tools.len(),
        shown.join(", "),
        suffix
    );
    if tree.tool_messages > 1 {
        summary.push_str(&format!(
            "  (⟳ +{} −{})",
            tree.tools_added, tree.tools_removed
        ));
    }
    summary
}

fn child(indent: &str, text: String, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(indent.to_string(), Style::default().fg(Color::DarkGray)),
        Span::styled(text, Style::default().fg(color)),
    ])
}

fn section_color(name: &str) -> Color {
    match name {
        "preamble" => Color::Cyan,
        "tools" | "rules" | "docs" => Color::Gray,
        "addendum" => Color::Magenta,
        "project_context" => Color::Yellow,
        "skills" => Color::Green,
        "cwd" => Color::DarkGray,
        _ => Color::DarkGray,
    }
}

fn fmt_chars(n: usize) -> String {
    if n < 1_000 {
        format!("{n} zn")
    } else if n < 10_000 {
        format!("{:.1}k zn", n as f64 / 1_000.0)
    } else {
        format!("{}k zn", n / 1_000)
    }
}

fn truncate(text: &str, max: usize) -> String {
    let mut out: String = text.chars().take(max).collect();
    if text.chars().count() > max {
        out.push('…');
    }
    out
}

/// Replace the home prefix with `~` and normalise separators.
fn shorten(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if let Some(home) = crate::shared::dirs_home() {
        let home = home.display().to_string().replace('\\', "/");
        if let Some(rest) = normalized.strip_prefix(&home) {
            return format!("~{rest}");
        }
    }
    normalized
}

#[cfg(test)]
mod tests;
