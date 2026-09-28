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

use sources::{attribute, file_state, iso_to_epoch_ms};

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

/// Drift of a loaded source file against the prompt that carried its text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SourceState {
    /// Still identical to what the prompt shows.
    #[default]
    Ok,
    /// Changed on disk *after* the prompt was built — pi only re-reads on
    /// `/reload`, so the running prompt is stale.
    Modified,
    /// Was loaded but is gone from disk now.
    Missing,
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
    /// Drift against the prompt that loaded it.
    pub state: SourceState,
}

/// Replayed system prompt of one session.
#[derive(Debug, Clone, Default)]
pub struct PromptTree {
    pub sections: Vec<PromptSection>,
    /// Every context file pi loaded, in the order it rendered them
    /// (agent directory first, then root → cwd).
    pub context_files: Vec<PromptFileRef>,
    /// `addendum` attribution (APPEND_SYSTEM.md), when the section exists,
    /// with the drift of the file it matched.
    pub append_system: Option<(PromptSource, SourceState)>,
    /// `SYSTEM.md` / `--system-prompt` attribution, when the default preamble
    /// was replaced.
    pub system_override: Option<(PromptSource, SourceState)>,
    /// Timestamp of the newest system message — the moment the effective
    /// prompt was assembled. Empty when the transcript has none.
    pub loaded_at: String,
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

/// Drift of a file loaded at `loaded_at` (an ISO-8601 timestamp).
///
/// Shared with the prompt sidecar, which knows the capture time from the engine
/// instead of from the transcript.
pub fn source_state(path: &str, loaded_at: &str) -> SourceState {
    sources::file_state(path, sources::iso_to_epoch_ms(loaded_at))
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
        builder.push_system_message(
            msg,
            entry.get("timestamp").and_then(Value::as_str).unwrap_or(""),
        );
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
    /// Timestamp of the newest system message, as pi wrote it.
    loaded_at: String,
    loaded_ms: u64,
}

impl Builder {
    fn push_system_message(&mut self, msg: &Value, timestamp: &str) {
        self.messages += 1;
        if !timestamp.is_empty() {
            self.loaded_at = timestamp.to_string();
            self.loaded_ms = iso_to_epoch_ms(timestamp);
        }

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

        let loaded_ms = self.loaded_ms;
        let preamble = self.text.get("preamble").cloned().unwrap_or_default();
        let has_tools = self.text.contains_key("tools");

        let attribute_source = |section: Option<&String>, file_name: &str| {
            attribute(section, cwd, file_name).map(|source| {
                let state = match &source {
                    PromptSource::File(path) => file_state(path, loaded_ms),
                    PromptSource::Inline => SourceState::Ok,
                };
                (source, state)
            })
        };

        let append_system = attribute_source(self.text.get("addendum"), "APPEND_SYSTEM.md");
        let system_override = if preamble.is_empty() || has_tools {
            None
        } else {
            attribute_source(Some(&preamble), "SYSTEM.md")
        };

        let mut context_files = self
            .text
            .get("project_context")
            .map(|text| parse_project_instructions(text))
            .unwrap_or_default();
        for file in &mut context_files {
            file.state = file_state(&file.path, loaded_ms);
        }

        PromptTree {
            sections,
            context_files,
            append_system,
            system_override,
            loaded_at: self.loaded_at,
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
            state: SourceState::Ok,
        });
        rest = next;
    }
    out
}

#[cfg(test)]
mod tests;
