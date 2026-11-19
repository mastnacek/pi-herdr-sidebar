//! System-prompt tree of a Claude Code session.
//!
//! Claude Code does not store its system prompt as one message; what it adds
//! to the model's context lands in the transcript as `attachment` entries.
//! They map onto the tree pi's replay builds:
//!
//! | attachment                          | section           |
//! |-------------------------------------|-------------------|
//! | `prompt_snapshot.systemPrompt`      | `preamble`        |
//! | `deferred_tools_delta`              | `tools` (+ list)  |
//! | `instructions.files[]` (CLAUDE.md)  | `project_context` |
//! | `skill_listing`                     | `skills`          |
//! | `environment.workingDirectory`      | `cwd`             |
//! | `agent_listing_delta`               | `agents`          |
//! | `mcp_instructions_delta`            | `mcp`             |
//! | `hook_additional_context` (start)   | `hooks`           |
//! | `session_context`                   | `session_context` |
//!
//! A type written again (a delta, a reload) counts as a patch of its section.
//! `session_context` carries the account e-mail: the tree shows its keys only.

use std::collections::BTreeMap;

use serde_json::Value;

use super::super::prompt_tree::{source_state, PromptFileRef, PromptSection, PromptTree, SECTION_ORDER};

#[derive(Default)]
struct Acc {
    chars: usize,
    preview: String,
    /// Writes that changed the section (`1` = written once).
    writes: u32,
}

impl Acc {
    /// A snapshot replaces the section (and is a patch only when its size
    /// changed); a delta adds to it and always counts.
    fn write(&mut self, chars: usize, preview: String, snapshot: bool) {
        if snapshot {
            if self.writes == 0 || self.chars != chars {
                self.writes += 1;
            }
            self.chars = chars;
            self.preview = preview;
            return;
        }
        self.chars += chars;
        if self.preview.is_empty() {
            self.preview = preview;
        }
        self.writes += 1;
    }
}

/// Builds the tree; `None` when the transcript holds no prompt attachment.
pub fn parse_prompt_tree(content: &str) -> Option<PromptTree> {
    let mut acc: BTreeMap<&'static str, Acc> = BTreeMap::new();
    let mut tree = PromptTree { claude: true, ..Default::default() };

    for line in content.lines() {
        let Ok(e) = serde_json::from_str::<Value>(line) else { continue };
        if e.get("type").and_then(Value::as_str) != Some("attachment") {
            continue;
        }
        let Some(a) = e.get("attachment") else { continue };
        let kind = a.get("type").and_then(Value::as_str).unwrap_or("");
        // A snapshot restates the whole section; a delta adds to it.
        let snapshot = matches!(kind, "prompt_snapshot" | "instructions" | "environment")
            || (kind == "skill_listing" && a.get("isInitial").and_then(Value::as_bool) == Some(true));
        let (name, text) = match kind {
            "prompt_snapshot" => ("preamble", strings(a.get("systemPrompt"))),
            "deferred_tools_delta" => {
                tree.tool_messages += 1;
                let added = names(a.get("addedNames"));
                let removed = names(a.get("removedNames"));
                tree.tools_added += added.len() as u32;
                tree.tools_removed += removed.len() as u32;
                tree.tools.retain(|t| !removed.contains(t));
                for t in added {
                    if !tree.tools.contains(&t) {
                        tree.tools.push(t);
                    }
                }
                ("tools", strings(a.get("addedLines")))
            }
            "instructions" => {
                let files = a.get("files").and_then(Value::as_array).cloned().unwrap_or_default();
                let at = e.get("timestamp").and_then(Value::as_str).unwrap_or("");
                tree.context_files = files
                    .iter()
                    .map(|f| {
                        let path = f.get("path").and_then(Value::as_str).unwrap_or("").to_string();
                        let chars = f.get("content").and_then(Value::as_str).map_or(0, |c| c.chars().count());
                        let state = source_state(&path, at);
                        PromptFileRef { path, chars, state }
                    })
                    .collect();
                let all: Vec<&str> = files.iter().filter_map(|f| f.get("content")?.as_str()).collect();
                ("project_context", all.join("\n"))
            }
            "skill_listing" => ("skills", str_of(a.get("content"))),
            "environment" => {
                let snap = a.get("snapshot");
                let cwd = snap.and_then(|s| s.get("workingDirectory"));
                ("cwd", str_of(cwd))
            }
            "agent_listing_delta" => ("agents", strings(a.get("addedLines"))),
            "mcp_instructions_delta" => ("mcp", strings(a.get("addedBlocks"))),
            "hook_additional_context" if a.get("hookEvent").and_then(Value::as_str) == Some("SessionStart") => {
                ("hooks", strings(a.get("content")))
            }
            "session_context" => {
                let ctx = a.get("context").and_then(Value::as_object);
                let keys: Vec<&str> = ctx.map(|c| c.keys().map(String::as_str).collect()).unwrap_or_default();
                let chars = ctx.map_or(0, |c| c.values().map(|v| str_of(Some(v)).chars().count()).sum());
                let slot = acc.entry("session_context").or_default();
                slot.write(chars, keys.join(", "), true);
                stamp(&mut tree, &e);
                continue;
            }
            _ => continue,
        };
        acc.entry(name).or_default().write(text.chars().count(), first_line(&text), snapshot);
        stamp(&mut tree, &e);
    }

    if acc.is_empty() {
        return None;
    }
    for (name, a) in acc {
        let patches = a.writes.saturating_sub(1);
        tree.patches += patches;
        tree.total_chars += a.chars;
        tree.sections.push(PromptSection {
            name: name.to_string(),
            chars: a.chars,
            preview: a.preview,
            patches,
            removed: false,
            custom: !SECTION_ORDER.contains(&name),
        });
    }
    Some(tree)
}

fn stamp(tree: &mut PromptTree, e: &Value) {
    if let Some(ts) = e.get("timestamp").and_then(Value::as_str) {
        tree.loaded_at = ts.to_string();
    }
}

fn str_of(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// An array of strings joined by newlines (a plain string passes through).
fn strings(v: Option<&Value>) -> String {
    match v {
        Some(Value::Array(items)) => items.iter().map(|i| str_of(Some(i))).collect::<Vec<_>>().join("\n"),
        other => str_of(other),
    }
}

fn names(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|n| n.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

fn first_line(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    line.chars().take(80).collect()
}
