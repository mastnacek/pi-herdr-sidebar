//! Independent Skills telemetry: derive the Skills face state directly from the
//! Pi session JSONL — the same source of truth the Status face uses. No TS
//! extension, sidecar, or event bus required.
//!
//! Replays pi-plugin-dev's tracker heuristics over the session tool-call trail:
//! - `read` on a `SKILL.md`          → skill activation
//! - `read` under `/references/`     → guidance reference loaded
//! - `read` in pi engine docs        → `doc_consult` action
//! - `read` of any other file        → inspected file
//! - `edit` / `write`                → modification + compliance gates
//! - `bash`                          → shell verification action

mod builder;
mod gates;
mod time;

pub use time::iso_to_epoch_ms;

use std::fs;
use std::path::Path;

use super::skills::{SkillSnapshotFile, SkillState};
use builder::Builder;

/// Actions kept for the Focus list (mirrors the TS publisher's `slice(-8)`).
pub(crate) const MAX_ACTIONS: usize = 8;

/// Parse a session JSONL file into Skills-face state. Never fails hard: an
/// empty/unknown file yields an empty live state rather than `None`.
pub fn parse_session_skills(path: &Path) -> Option<SkillSnapshotFile> {
    let content = fs::read_to_string(path).ok()?;
    Some(parse_skills_str(&content))
}

/// Parse session JSONL content and derive the Skills face state.
pub fn parse_skills_str(content: &str) -> SkillSnapshotFile {
    let mut b = Builder::new();

    for line in content.lines() {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if entry.get("type").and_then(serde_json::Value::as_str) != Some("message") {
            continue;
        }
        let Some(msg) = entry.get("message") else {
            continue;
        };
        let role = msg.get("role").and_then(serde_json::Value::as_str).unwrap_or("");
        let ts = entry
            .get("timestamp")
            .and_then(serde_json::Value::as_str)
            .and_then(time::iso_to_epoch_ms)
            .unwrap_or(0);

        if role == "user" {
            // A user message starts a new agent turn.
            b.turn_count += 1;
            b.in_turn = true;
            b.last_ts = ts;
            continue;
        }
        if role == "toolResult" {
            // Tool output returned: the agent is still mid-turn.
            b.in_turn = true;
            b.last_ts = ts;
            continue;
        }
        if role != "assistant" {
            continue;
        }

        let Some(blocks) = msg.get("content").and_then(serde_json::Value::as_array) else {
            continue;
        };
        b.last_ts = ts;

        // A settled turn ends with a plain text/thinking assistant message.
        let mut has_tool_call = false;
        for block in blocks {
            if block.get("type").and_then(serde_json::Value::as_str) != Some("toolCall") {
                continue;
            }
            has_tool_call = true;
            let name = block.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
            let args = block.get("arguments").cloned().unwrap_or(serde_json::Value::Null);
            b.on_tool_call(name, &args, ts);
        }
        if !has_tool_call {
            b.in_turn = false;
        }
        b.last_assistant_had_tool_call = has_tool_call;
        b.entry_index += 1;
    }

    wrap(b.finish())
}

fn wrap(state: SkillState) -> SkillSnapshotFile {
    SkillSnapshotFile {
        version: 1,
        live: true,
        generated_at: String::new(),
        state: Some(state),
    }
}

#[cfg(test)]
mod tests;
