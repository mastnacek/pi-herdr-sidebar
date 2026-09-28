//! Transcript replay: fold every `role: "system"` message into the prompt
//! the model actually received.

use std::collections::HashMap;

use serde_json::Value;

use super::sources::{attribute, file_state, iso_to_epoch_ms};
use super::text::{parse_project_instructions, preview, strip_wrapper};
use super::{PromptSection, PromptSource, PromptTree, SourceState, SECTION_ORDER};

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
            // Sizes are the *inner* text: the transcript wraps every section
            // except `preamble` as `<name>…</name>`, and the exact provider
            // measures the same way, so both modes report comparable numbers.
            let inner = strip_wrapper(&name, text);
            let chars = if removed { 0 } else { inner.chars().count() };
            total_chars += chars;
            sections.push(PromptSection {
                name: name.clone(),
                chars,
                preview: preview(&inner, &name),
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

        let append_system = {
            // Attribute the *body*: the transcript stores every section wrapped
            // as `<name>…</name>`, while a file holds the text alone.
            let body = self
                .text
                .get("addendum")
                .map(|text| strip_wrapper("addendum", text));
            attribute_source(body.as_ref(), "APPEND_SYSTEM.md")
        };
        let system_override = if preamble.is_empty() || has_tools {
            None
        } else {
            let body = strip_wrapper("preamble", &preamble);
            attribute_source(Some(&body), "SYSTEM.md")
        };

        // The previews are what make an inline append recognisable without the
        // exact provider: the transcript carries the text, just never the path.
        let addendum_preview = self
            .text
            .get("addendum")
            .map(|text| preview(&strip_wrapper("addendum", text), "addendum"))
            .unwrap_or_default();
        let override_preview = if system_override.is_some() {
            preview(&preamble, "preamble")
        } else {
            String::new()
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
            addendum_preview,
            system_override,
            override_preview,
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
