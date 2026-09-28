//! System-prompt provenance for the Status face.
//!
//! Pi persists the *system prompt itself* in the session transcript
//! (`pi-coding-agent/docs/session-format.md`): the first request writes a
//! `role: "system"` message whose `sections` field carries every prompt
//! section, and later changes arrive as further system messages that patch
//! sections by name (`null` removes one) and list `toolsAdded` /
//! `toolsRemoved`. Replaying them in order yields the exact prompt the model
//! received — so the sidebar can show which `AGENTS.md` files were loaded and
//! whether an `APPEND_SYSTEM.md` addendum made it into the request, without
//! re-discovering or guessing anything.
//!
//! Section order mirrors `dist/core/system-prompt.js#buildSystemPromptSections`:
//!
//! 1. `preamble`          base instructions, or `SYSTEM.md` / `--system-prompt`
//! 2. `tools`             one-line snippet per selected tool
//! 3. `rules`             tool + prompt guidelines
//! 4. `docs`              pointers into the pi documentation tree
//! 5. `addendum`          `APPEND_SYSTEM.md` (+ `--append-system-prompt`)
//! 6. `project_context`   every discovered `AGENTS.md` / `CLAUDE.md`
//! 7. `skills`            loaded skill descriptions
//! 8. `cwd`               resolved working directory
//!
//! Extension-supplied sections are appended after `cwd` by pi. `preamble` is
//! the only raw section; every other section reaches the model wrapped as
//! `<name>…</name>`.
//!
//! serde_json has no `preserve_order` feature here, so a JSON object iterates
//! alphabetically. The documented build order above is used instead, which is
//! also what the transcript's insertion order encodes.

use std::collections::HashMap;

use serde_json::Value;

mod sources;

use sources::attribute;

/// Pi's section build order (see the module docs). Unknown names are
/// extension sections and render after these.
pub const SECTION_ORDER: [&str; 8] = [
    "preamble",
    "tools",
    "rules",
    "docs",
    "addendum",
    "project_context",
    "skills",
    "cwd",
];

/// Where a section's text came from, when it could be attributed to a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptSource {
    /// A file on disk whose content equals (or is a suffix of) the section.
    File(String),
    /// No file matched — inline `--system-prompt` / `--append-system-prompt`,
    /// a joined combination, or an extension.
    Inline,
}

/// One prompt section after replaying every patch.
#[derive(Debug, Clone, Default)]
pub struct PromptSection {
    pub name: String,
    /// Effective character count (`0` when the section was removed).
    pub chars: usize,
    /// First non-empty line, trimmed for the tree.
    pub preview: String,
    /// How many system messages wrote this section (`0` = one write).
    pub patches: u32,
    /// A later system message removed the section with `null`.
    pub removed: bool,
    /// Not part of pi's documented build order (extension-supplied).
    pub custom: bool,
}

/// An `<project_instructions path="…">` entry inside `project_context`.
#[derive(Debug, Clone, Default)]
pub struct PromptFileRef {
    pub path: String,
    pub chars: usize,
}

/// Replayed system prompt of one session.
#[derive(Debug, Clone, Default)]
pub struct PromptTree {
    pub sections: Vec<PromptSection>,
    /// Every context file pi loaded, in the order it rendered them
    /// (agent directory first, then root → cwd).
    pub context_files: Vec<PromptFileRef>,
    /// `addendum` attribution (APPEND_SYSTEM.md), when the section exists.
    pub append_system: Option<PromptSource>,
    /// `APPEND_SYSTEM.md` candidates found on disk but not present in the
    /// prompt (e.g. project file when the project is untrusted).
    pub append_candidates: Vec<String>,
    /// `SYSTEM.md` / `--system-prompt` attribution, when the default preamble
    /// was replaced.
    pub system_override: Option<PromptSource>,
    pub system_candidates: Vec<String>,
    /// Effective tool loadout after `toolsAdded` / `toolsRemoved` replay.
    pub tools: Vec<String>,
    pub tools_added: u32,
    pub tools_removed: u32,
    /// System messages that declared a tool loadout (first request + patches).
    pub tool_messages: u32,
    /// System messages after the first one (each patches something).
    pub patches: u32,
    pub total_chars: usize,
}

/// Parse a session JSONL body into the replayed system-prompt tree.
///
/// Returns `None` when the transcript carries no system message yet (a session
/// whose first request has not been sent).
pub fn parse_prompt_tree(content: &str, cwd: &str) -> Option<PromptTree> {
    let mut builder = Builder::default();

    for line in content.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if entry.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let Some(msg) = entry.get("message") else {
            continue;
        };
        if msg.get("role").and_then(Value::as_str) != Some("system") {
            continue;
        }
        builder.push_system_message(msg);
    }

    if builder.messages == 0 {
        return None;
    }

    Some(builder.finish(cwd))
}

#[derive(Default)]
struct Builder {
    messages: u32,
    /// Section name → effective text (`""` for removed sections).
    text: HashMap<String, String>,
    /// Section name → number of writes (first write + patches).
    writes: HashMap<String, u32>,
    removed: HashMap<String, bool>,
    /// First-appearance order, for extension sections.
    order: Vec<String>,
    tools: Vec<String>,
    tools_added: u32,
    tools_removed: u32,
    tool_messages: u32,
}

impl Builder {
    fn push_system_message(&mut self, msg: &Value) {
        self.messages += 1;

        if let Some(sections) = msg.get("sections").and_then(Value::as_object) {
            for (name, value) in sections {
                if !self.order.iter().any(|n| n == name) {
                    self.order.push(name.clone());
                }
                let writes = self.writes.entry(name.clone()).or_insert(0);
                *writes += 1;
                match value {
                    Value::Null => {
                        self.removed.insert(name.clone(), true);
                        self.text.insert(name.clone(), String::new());
                    }
                    Value::String(text) => {
                        self.removed.insert(name.clone(), false);
                        self.text.insert(name.clone(), text.clone());
                    }
                    _ => {}
                }
            }
        }

        let has_tools_added = msg.get("toolsAdded").and_then(Value::as_array).is_some();
        let has_tools_removed = msg.get("toolsRemoved").and_then(Value::as_array).is_some();
        if has_tools_added || has_tools_removed {
            self.tool_messages += 1;
        }
        if let Some(added) = msg.get("toolsAdded").and_then(Value::as_array) {
            for tool in added {
                let Some(name) = tool.get("name").and_then(Value::as_str) else {
                    continue;
                };
                self.tools_added += 1;
                if !self.tools.iter().any(|t| t == name) {
                    self.tools.push(name.to_string());
                }
            }
        }
        if let Some(removed) = msg.get("toolsRemoved").and_then(Value::as_array) {
            for tool in removed {
                let Some(name) = tool.get("name").and_then(Value::as_str) else {
                    continue;
                };
                self.tools_removed += 1;
                self.tools.retain(|t| t != name);
            }
        }
    }

    fn finish(self, cwd: &str) -> PromptTree {
        let mut sections = Vec::new();
        let mut total_chars = 0usize;

        let ordered = SECTION_ORDER.iter().map(|n| n.to_string()).chain(
            self.order
                .iter()
                .filter(|n| !SECTION_ORDER.contains(&n.as_str()))
                .cloned(),
        );

        for name in ordered {
            let Some(text) = self.text.get(&name) else {
                continue;
            };
            let removed = self.removed.get(&name).copied().unwrap_or(false);
            let chars = if removed { 0 } else { text.chars().count() };
            total_chars += chars;
            sections.push(PromptSection {
                name: name.clone(),
                chars,
                preview: preview(text, &name),
                patches: self
                    .writes
                    .get(&name)
                    .copied()
                    .unwrap_or(1)
                    .saturating_sub(1),
                removed,
                custom: !SECTION_ORDER.contains(&name.as_str()),
            });
        }

        let preamble = self.text.get("preamble").cloned().unwrap_or_default();
        let has_tools = self.text.contains_key("tools");

        let (append_system, append_candidates) =
            attribute(self.text.get("addendum"), cwd, "APPEND_SYSTEM.md");
        let (system_override, system_candidates) = if preamble.is_empty() || has_tools {
            (None, Vec::new())
        } else {
            let (source, candidates) = attribute(Some(&preamble), cwd, "SYSTEM.md");
            (source, candidates)
        };

        let context_files = self
            .text
            .get("project_context")
            .map(|text| parse_project_instructions(text))
            .unwrap_or_default();

        PromptTree {
            sections,
            context_files,
            append_system,
            append_candidates,
            system_override,
            system_candidates,
            tools: self.tools,
            tools_added: self.tools_added,
            tools_removed: self.tools_removed,
            tool_messages: self.tool_messages,
            patches: self.messages.saturating_sub(1),
            total_chars,
        }
    }
}

/// First meaningful line of a section body, skipping the `<name>…</name>`
/// wrapper pi adds.
fn preview(text: &str, name: &str) -> String {
    let open = format!("<{name}>");
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| {
            !l.is_empty() && *l != open && *l != "Project-specific instructions and guidelines:"
        })
        .unwrap_or("");
    let mut out: String = line.chars().take(64).collect();
    if line.chars().count() > 64 {
        out.push('…');
    }
    out
}

/// Extract `<project_instructions path="…">…</project_instructions>` entries
/// from the `project_context` section.
fn parse_project_instructions(text: &str) -> Vec<PromptFileRef> {
    const OPEN: &str = "<project_instructions path=\"";
    const CLOSE: &str = "</project_instructions>";

    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        let after = &rest[start + OPEN.len()..];
        let Some(quote) = after.find('"') else {
            break;
        };
        let path = after[..quote].to_string();
        let Some(gt) = after[quote..].find('>') else {
            break;
        };
        let body = &after[quote + gt + 1..];
        let (content, next) = match body.find(CLOSE) {
            Some(end) => (&body[..end], &body[end + CLOSE.len()..]),
            None => (body, ""),
        };
        out.push(PromptFileRef {
            path,
            chars: content.trim().chars().count(),
        });
        rest = next;
    }
    out
}

#[cfg(test)]
mod tests;
