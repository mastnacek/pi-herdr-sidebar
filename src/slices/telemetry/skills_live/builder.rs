//! Session-replay builder: folds the session tool-call trail into Skills-face
//! state (activation, references, focus actions, counters).

use serde_json::Value;
use std::collections::HashMap;

use super::super::skills::{ComplianceCheck, SkillAction, SkillReference, SkillState};
use super::gates::run_gates;
use super::MAX_ACTIONS;

/// Incrementally derived Skills state from one session JSONL replay.
pub(super) struct Builder {
    pub active_skill: Option<String>,
    pub references: HashMap<String, SkillReference>,
    pub actions: Vec<SkillAction>,
    pub compliance: Vec<ComplianceCheck>,
    pub inspected: u64,
    pub modified: u64,
    pub start_ms: Option<u64>,
    pub last_ts: u64,
    pub turn_count: u64,
    pub in_turn: bool,
    /// True when the newest assistant message carried a tool call (mid-turn).
    pub last_assistant_had_tool_call: bool,
    pub entry_index: usize,
}

impl Builder {
    pub(super) fn new() -> Self {
        Self {
            active_skill: None,
            references: HashMap::new(),
            actions: Vec::new(),
            compliance: Vec::new(),
            inspected: 0,
            modified: 0,
            start_ms: None,
            last_ts: 0,
            turn_count: 0,
            in_turn: false,
            last_assistant_had_tool_call: false,
            entry_index: 0,
        }
    }

    /// Replay one tool call against the tracker heuristics.
    pub(super) fn on_tool_call(&mut self, tool_name: &str, args: &Value, ts: u64) {
        if ts > 0 {
            if self.start_ms.is_none() {
                self.start_ms = Some(ts);
            }
            self.last_ts = ts;
        }
        let raw_path = args.get("path").and_then(Value::as_str).unwrap_or("");
        let norm = raw_path.replace('\\', "/");
        let base = norm.rsplit('/').find(|s| !s.is_empty()).unwrap_or("");

        // 1. Skill activation via SKILL.md read.
        if tool_name == "read" && norm.ends_with("SKILL.md") {
            self.active_skill = skill_name_from(&norm);
            self.push_action(
                "read",
                base_label(&norm),
                format!(
                    "Loaded skill entry point: {}",
                    self.active_skill.clone().unwrap_or_default()
                ),
                ts,
            );
            return;
        }

        // 2. Guidance reference document read.
        if tool_name == "read" && norm.contains("/references/") {
            self.references
                .entry(base.to_string())
                .or_insert(SkillReference {
                    name: base.to_string(),
                    summary: reference_summary(base).to_string(),
                });
            self.push_action(
                "read",
                base,
                format!("Guidance: {}", reference_summary(base)),
                ts,
            );
            return;
        }

        // 3. Engine docs consultation.
        if tool_name == "read" && norm.contains("@earendil-works/pi-coding-agent/docs") {
            self.push_action("doc_consult", base, format!("Engine API doc: {base}"), ts);
            return;
        }

        // 4. Plain file inspection.
        if tool_name == "read" && !raw_path.is_empty() {
            self.inspected += 1;
            self.push_action("read", base, format!("Inspecting {base}"), ts);
            return;
        }

        // 5. Modification → compliance gates.
        if tool_name == "edit" || tool_name == "write" {
            if !raw_path.is_empty() {
                self.modified += 1;
            }
            let payload = match tool_name {
                "write" => args
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                "edit" => args
                    .get("edits")
                    .and_then(Value::as_array)
                    .map(|edits| {
                        edits
                            .iter()
                            .filter_map(|e| e.get("newText").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default(),
                _ => String::new(),
            };
            run_gates(&norm, &payload, ts, &mut self.compliance);
            let verb = if tool_name == "edit" {
                "Modifying"
            } else {
                "Writing"
            };
            self.push_action(tool_name, base, format!("{verb} {base}"), ts);
            return;
        }

        // 6. Bash verification commands.
        if tool_name == "bash" {
            if let Some(cmd) = args.get("command").and_then(Value::as_str) {
                let target: String = cmd.chars().take(40).collect();
                let summary: String = cmd.chars().take(48).collect();
                self.push_action("bash", &target, format!("Shell: {summary}"), ts);
            }
        }
    }

    pub(super) fn push_action(&mut self, kind: &str, target: &str, summary: String, ts: u64) {
        self.actions.push(SkillAction {
            kind: kind.to_string(),
            target: target.to_string(),
            summary,
            timestamp: ts,
        });
    }

    pub(super) fn finish(self) -> SkillState {
        let start = self.start_ms.unwrap_or(self.last_ts);
        SkillState {
            active_skill: self.active_skill,
            references: self.references.into_values().collect(),
            actions: {
                let mut a = self.actions;
                if a.len() > MAX_ACTIONS {
                    a.drain(..a.len() - MAX_ACTIONS);
                }
                a
            },
            compliance: self.compliance,
            inspected_count: self.inspected,
            modified_count: self.modified,
            start_time: start,
            last_update_time: self.last_ts,
            in_turn: self.in_turn,
            turn_count: self.turn_count,
        }
    }
}

/// Skill name: the path segment directly before `SKILL.md`.
pub(super) fn skill_name_from(norm_path: &str) -> Option<String> {
    let segments: Vec<&str> = norm_path.split('/').filter(|s| !s.is_empty()).collect();
    let idx = segments.iter().position(|s| *s == "SKILL.md")?;
    if idx > 0 {
        Some(segments[idx - 1].to_string())
    } else {
        Some("pi-skill".to_string())
    }
}

pub(super) fn base_label(norm_path: &str) -> &str {
    norm_path.rsplit('/').next().unwrap_or(norm_path)
}

/// Guidance summary heuristics, ported from pi-plugin-dev's tracker.
pub(super) fn reference_summary(filename: &str) -> &'static str {
    let lower = filename.to_lowercase();
    if lower.contains("command-completion") {
        "Trailing Space Contract & Autocomplete Engine"
    } else if lower.contains("tools-and-schema") {
        "StringEnum Rule & Tool Execution Contract"
    } else if lower.contains("lifecycle") {
        "Lifecycle Events & Engine Compatibility"
    } else if lower.contains("state-persistence") {
        "Branch-Aware Session State & Global Config"
    } else if lower.contains("api-docs") {
        "Live Local Engine Documentation Index"
    } else {
        "Skill Guidance Reference Document"
    }
}

