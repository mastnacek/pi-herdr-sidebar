//! Rendering primitives for the Status-face system-prompt tree.
//!
//! Split out of `prompt_tree_view.rs` to keep both files under the line limit;
//! everything here is presentation only — the view-model and the provider
//! selection live in the parent module.

use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};

use crate::slices::telemetry::prompt_tree::{PromptSection, PromptSource, SourceState};

use super::PromptView;

/// A section row: `├─ 3 rules  4.1k zn ⟳1  preview…`.
pub(super) fn section_line(
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
            format!("· odstraněno pozdějším patchem ({})", super::hint_for(name)),
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
pub(super) fn child_lines(
    glyph: &str,
    section: &PromptSection,
    view: &PromptView,
) -> Vec<Line<'static>> {
    let indent = if glyph == "└─" {
        "     "
    } else {
        "│    "
    };
    let mut lines = Vec::new();

    match section.name.as_str() {
        "addendum" => match view.append_system.as_ref() {
            Some((PromptSource::File(path), state)) => lines.push(child_with_state(
                indent,
                format!("← {}{}", shorten(path), size_suffix(view.append_chars)),
                Color::Magenta,
                *state,
            )),
            Some((PromptSource::Inline, _)) => lines.push(child(
                indent,
                format!(
                    "← inline{}{}",
                    preview_suffix(&view.append_preview),
                    size_suffix(view.append_chars)
                ),
                Color::Magenta,
            )),
            None => {}
        },
        "preamble" => match view.system_override.as_ref() {
            Some((PromptSource::File(path), state)) => lines.push(child_with_state(
                indent,
                format!(
                    "⚠ nahrazeno: {}{}",
                    shorten(path),
                    size_suffix(view.override_chars)
                ),
                Color::Yellow,
                *state,
            )),
            Some((PromptSource::Inline, _)) => lines.push(child(
                indent,
                format!(
                    "⚠ nahrazeno: inline{}{}",
                    preview_suffix(&view.override_preview),
                    size_suffix(view.override_chars)
                ),
                Color::Yellow,
            )),
            None => {}
        },
        "project_context" => {
            for (offset, file) in view.context_files.iter().enumerate() {
                let branch = if offset + 1 == view.context_files.len() {
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
            lines.push(child(indent, tool_summary(view), Color::DarkGray));
        }
        "skills" => {
            if let Some(summary) = skill_summary(view) {
                lines.push(child(indent, summary, Color::Green));
            }
        }
        _ => {}
    }

    lines
}

/// `HH:MM:SS` of the capture — the moment the effective prompt was assembled.
pub(super) fn loaded_suffix(loaded_at: &str) -> String {
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
fn child_with_state(indent: &str, text: String, color: Color, state: SourceState) -> Line<'static> {
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

fn tool_summary(view: &PromptView) -> String {
    if view.tools.is_empty() {
        return "· bez deklarovaných nástrojů".to_string();
    }
    let shown: Vec<&str> = view.tools.iter().take(5).map(|s| s.as_str()).collect();
    let rest = view.tools.len().saturating_sub(shown.len());
    let suffix = if rest > 0 {
        format!(" +{rest}")
    } else {
        String::new()
    };
    let mut summary = format!(
        "· {} nástrojů: {}{}",
        view.tools.len(),
        shown.join(", "),
        suffix
    );
    if view.tool_messages > 1 {
        summary.push_str(&format!(
            "  (⟳ +{} −{})",
            view.tools_added, view.tools_removed
        ));
    }
    summary
}

/// Skill names as the engine resolved them. Only the sidecar knows them; the
/// transcript replay has just the rendered `skills` section.
fn skill_summary(view: &PromptView) -> Option<String> {
    if view.skills.is_empty() {
        return None;
    }
    let shown: Vec<&str> = view.skills.iter().take(4).map(|s| s.as_str()).collect();
    let rest = view.skills.len().saturating_sub(shown.len());
    let suffix = if rest > 0 {
        format!(" +{rest}")
    } else {
        String::new()
    };
    Some(format!(
        "· {} skillů{}: {}{}",
        view.skills.len(),
        size_suffix(view.skill_chars),
        shown.join(", "),
        suffix
    ))
}

/// ` · "CIM BUDU…"` — names an inline block the engine gave no path for.
fn preview_suffix(preview: &str) -> String {
    if preview.is_empty() {
        String::new()
    } else {
        format!(" · \"{preview}\"")
    }
}

/// ` (312 zn)` when the provider reported a size, empty otherwise.
fn size_suffix(chars: usize) -> String {
    if chars == 0 {
        String::new()
    } else {
        format!(" ({})", fmt_chars(chars))
    }
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
