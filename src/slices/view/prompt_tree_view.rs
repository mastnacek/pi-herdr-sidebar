//! System-prompt tree for the Status face.
//!
//! Renders the prompt sections in pi's documented build order — the frame is
//! always complete, so an absent `APPEND_SYSTEM.md` addendum is as visible as a
//! present one. Sources (`AGENTS.md` paths, the matching `APPEND_SYSTEM.md` /
//! `SYSTEM.md` file) hang below their section.
//!
//! Two providers feed the same tree:
//!
//! * the `.prompt.json` sidecar — `before_agent_start` state captured by the TS
//!   extension, labelled `exact`: real paths, forced prompts, CLI appends;
//! * the session transcript — labelled `replay`: no extension needed, but
//!   section sources are inferred and a forced prompt is invisible.

use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};

use crate::slices::telemetry::prompt_sidecar::PromptSidecar;
use crate::slices::telemetry::prompt_tree::{
    PromptFileRef, PromptSection, PromptSource, PromptTree, SourceState, SECTION_ORDER,
};
use crate::slices::telemetry::LiveTelemetry;

mod render;

use render::{child_lines, loaded_suffix, section_line};

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

/// Provider-independent shape the tree renders.
#[derive(Default)]
struct PromptView {
    sections: Vec<PromptSection>,
    context_files: Vec<PromptFileRef>,
    append_system: Option<(PromptSource, SourceState)>,
    /// Character count of the addendum text, when the provider reported it.
    append_chars: usize,
    /// First line of the addendum text — names an inline (CLI) append.
    append_preview: String,
    system_override: Option<(PromptSource, SourceState)>,
    /// Character count of the `SYSTEM.md` / forced prompt text.
    override_chars: usize,
    /// First line of the replacement text.
    override_preview: String,
    loaded_at: String,
    tools: Vec<String>,
    tools_added: u32,
    tools_removed: u32,
    tool_messages: u32,
    skills: Vec<String>,
    /// Total description characters across loaded skills, when known.
    skill_chars: usize,
    patches: u32,
    total_chars: usize,
    /// `exact` (engine sidecar) or `replay` (session transcript).
    exact: bool,
    /// Set when the whole prompt was replaced by a forced one.
    forced: bool,
}

impl PromptView {
    fn from_sidecar(sidecar: &PromptSidecar) -> Self {
        let ordered = SECTION_ORDER.iter().map(|n| n.to_string()).chain(
            sidecar
                .sections
                .iter()
                .filter(|n| !SECTION_ORDER.contains(&n.as_str()))
                .cloned(),
        );

        let mut sections = Vec::new();
        for name in ordered {
            let Some(chars) = sidecar.section_chars(&name) else {
                continue;
            };
            sections.push(PromptSection {
                chars,
                custom: !SECTION_ORDER.contains(&name.as_str()),
                name,
                ..Default::default()
            });
        }

        Self {
            sections,
            context_files: sidecar
                .context_files
                .iter()
                .map(|file| PromptFileRef {
                    state: sidecar.file_state(&file.path),
                    path: file.path.clone(),
                    chars: file.chars,
                })
                .collect(),
            append_system: sidecar.attribute(&sidecar.append_system_prompt),
            append_chars: sidecar
                .append_system_prompt
                .as_ref()
                .map(|t| t.chars)
                .unwrap_or(0),
            append_preview: sidecar
                .append_system_prompt
                .as_ref()
                .map(|t| t.preview.clone())
                .unwrap_or_default(),
            system_override: sidecar.attribute(&sidecar.custom_prompt),
            override_chars: sidecar.custom_prompt.as_ref().map(|t| t.chars).unwrap_or(0),
            override_preview: sidecar
                .custom_prompt
                .as_ref()
                .map(|t| t.preview.clone())
                .unwrap_or_default(),
            loaded_at: sidecar.captured_at.clone(),
            tools: sidecar.tools.clone(),
            skills: sidecar.skills.iter().map(|s| s.name.clone()).collect(),
            skill_chars: sidecar.skills.iter().map(|s| s.description_chars).sum(),
            total_chars: sidecar.system_prompt_chars,
            exact: true,
            forced: sidecar.forced,
            ..Default::default()
        }
    }

    fn from_replay(tree: &PromptTree) -> Self {
        Self {
            sections: tree.sections.clone(),
            context_files: tree.context_files.clone(),
            append_system: tree.append_system.clone(),
            append_chars: 0,
            append_preview: String::new(),
            system_override: tree.system_override.clone(),
            override_chars: 0,
            override_preview: String::new(),
            loaded_at: tree.loaded_at.clone(),
            tools: tree.tools.clone(),
            tools_added: tree.tools_added,
            tools_removed: tree.tools_removed,
            tool_messages: tree.tool_messages,
            skills: Vec::new(),
            skill_chars: 0,
            patches: tree.patches,
            total_chars: tree.total_chars,
            exact: false,
            forced: false,
        }
    }
}

/// Build the Status-face lines for the system prompt.
///
/// The sidecar wins when it is live: it is what the engine is about to send,
/// so it also covers forced prompts and CLI appends. The transcript replay is
/// the always-available fallback.
pub(super) fn prompt_tree_lines(
    t: &LiveTelemetry,
    sidecar: Option<&PromptSidecar>,
) -> Vec<Line<'static>> {
    let view = match sidecar {
        Some(sidecar) => Some(PromptView::from_sidecar(sidecar)),
        None => t.prompt.as_ref().map(PromptView::from_replay),
    };

    let Some(view) = view else {
        return vec![
            Line::from(vec![
                Span::styled(
                    "🧠 Systémový prompt: ",
                    Style::default().fg(Color::Cyan).bold(),
                ),
                Span::styled(
                    "čekám na první tah (session log i sidecar jsou prázdné)",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::raw(""),
        ];
    };

    let mut lines = Vec::new();
    let source_label = if view.exact {
        "exact · before_agent_start"
    } else {
        "replay ze session logu"
    };
    let badge_color = if view.exact {
        Color::Green
    } else {
        Color::DarkGray
    };

    let mut header = vec![
        Span::styled(
            "🧠 Systémový prompt ",
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::styled(
            format!(
                "({source_label} · {} sekcí · {}{})",
                view.sections.len(),
                fmt_chars(view.total_chars),
                loaded_suffix(&view.loaded_at)
            ),
            Style::default().fg(badge_color),
        ),
    ];
    if view.forced {
        header.push(Span::styled(
            "  ⚠ vynucený prompt",
            Style::default().fg(Color::Red).bold(),
        ));
    }
    if view.patches > 0 {
        header.push(Span::styled(
            format!("  ⟳ {} pozdějších patchů", view.patches),
            Style::default().fg(Color::Yellow),
        ));
    }
    lines.push(Line::from(header));

    let custom: Vec<&PromptSection> = view.sections.iter().filter(|s| s.custom).collect();
    let total = SECTION_ORDER.len();
    for (index, name) in SECTION_ORDER.iter().enumerate() {
        let glyph = if index + 1 == total && custom.is_empty() {
            "└─"
        } else {
            "├─"
        };
        let section = view.sections.iter().find(|s| s.name == *name);
        lines.push(section_line(glyph, index + 1, name, section));
        if let Some(section) = section {
            lines.extend(child_lines(glyph, section, &view));
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

fn fmt_chars(n: usize) -> String {
    if n < 1_000 {
        format!("{n} zn")
    } else if n < 10_000 {
        format!("{:.1}k zn", n as f64 / 1_000.0)
    } else {
        format!("{}k zn", n / 1_000)
    }
}

#[cfg(test)]
mod tests;
