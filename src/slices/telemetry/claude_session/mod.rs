//! Claude Code session telemetry.
//!
//! Herdr reports a Claude pane as `agent: "claude"` with
//! `agent_session {kind: "id", value: "<uuid>"}`; Claude Code keeps that
//! session's transcript at `~/.claude/projects/<cwd-slug>/<uuid>.jsonl`.
//! This module finds the file and fills the same [`LiveTelemetry`] the pi
//! parser does, so every face renders a Claude pane without knowing the agent.
//!
//! Transcript quirks handled here:
//! - one API response is written as several `assistant` lines (one per content
//!   block) that repeat the same `message.usage`; usage is counted once per
//!   `message.id`;
//! - `isSidechain` lines belong to a subagent: they cost money but are not the
//!   main context;
//! - a `system` line with `subtype: "compact_boundary"` invalidates the last
//!   usage as a context reading until the next response.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::git_live::git_info;
use super::model_catalog::{catalog_cost, context_window_for};
use super::{LiveTelemetry, Usage};

/// Provider name used for catalog lookups (pi's `models-store.json` lists
/// Claude models under it).
const PROVIDER: &str = "anthropic";

#[derive(Deserialize, Default)]
struct ClaudeUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_creation: Option<CacheCreation>,
    #[serde(default)]
    output_tokens_details: Option<OutputDetails>,
}

#[derive(Deserialize, Default)]
struct CacheCreation {
    #[serde(default)]
    ephemeral_1h_input_tokens: u64,
}

#[derive(Deserialize, Default)]
struct OutputDetails {
    #[serde(default)]
    thinking_tokens: u64,
}

#[derive(Deserialize, Default)]
struct ClaudeMessage {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    usage: Option<ClaudeUsage>,
    #[serde(default)]
    content: Option<serde_json::Value>,
}

#[derive(Deserialize, Default)]
struct ClaudeEntry {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default)]
    timestamp: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default, rename = "sessionId")]
    session_id: Option<String>,
    #[serde(default)]
    effort: Option<String>,
    #[serde(default, rename = "isSidechain")]
    is_sidechain: bool,
    #[serde(default, rename = "isMeta")]
    is_meta: bool,
    #[serde(default, rename = "isCompactSummary")]
    is_compact_summary: bool,
    #[serde(default)]
    message: Option<ClaudeMessage>,
}

/// `D:\01_programovani\dejaview` → `D--01-programovani-dejaview`: Claude Code
/// replaces every character that is not an ASCII letter or digit with `-`.
pub fn project_slug(cwd: &str) -> String {
    cwd.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Root of Claude Code transcripts: `~/.claude/projects`.
fn projects_root() -> Option<PathBuf> {
    Some(crate::shared::dirs_home()?.join(".claude").join("projects"))
}

/// Finds `<uuid>.jsonl`: first in the cwd's own project folder, then in any.
pub fn find_session_file(session_id: &str, cwd: Option<&str>) -> Option<PathBuf> {
    find_session_file_in(&projects_root()?, session_id, cwd)
}

fn find_session_file_in(root: &Path, session_id: &str, cwd: Option<&str>) -> Option<PathBuf> {
    if session_id.len() < 8 {
        return None;
    }
    let name = format!("{session_id}.jsonl");
    if let Some(cwd) = cwd {
        let scoped = root.join(project_slug(cwd)).join(&name);
        if scoped.is_file() {
            return Some(scoped);
        }
    }
    fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|d| d.path().join(&name))
        .find(|p| p.is_file())
}

/// True for a file under `~/.claude/projects` — the parser to use for it.
pub fn is_transcript(path: &Path) -> bool {
    let mut parts = path.components().rev().map(|c| c.as_os_str().to_string_lossy());
    parts.nth(2).as_deref() == Some("projects") && parts.next().as_deref() == Some(".claude")
}

/// Parses a Claude Code transcript; `None` when it holds no model response.
pub fn parse_session(path: &Path, session_id: &str) -> Option<LiveTelemetry> {
    let content = fs::read_to_string(path).ok()?;
    let mut t = parse_content(&content, session_id)?;
    t.session_file = Some(path.to_path_buf());
    // Display only: catalog lookups use `PROVIDER`, never this field.
    t.provider = subscription::label().to_string();
    if !t.cwd.is_empty() {
        if let Some(git) = git_info(Path::new(&t.cwd)) {
            t.git_branch = git.branch.clone();
            t.git_dirty = git.staged + git.unstaged + git.untracked;
            t.git = Some(git);
        }
    }
    Some(t)
}

fn parse_content(content: &str, session_id: &str) -> Option<LiveTelemetry> {
    let mut t = LiveTelemetry {
        session_id: session_id.to_string(),
        provider: PROVIDER.to_string(),
        ..Default::default()
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut context: Option<u64> = None;
    let mut working = false;

    for line in content.lines() {
        let Ok(e) = serde_json::from_str::<ClaudeEntry>(line) else {
            continue;
        };
        if let Some(c) = e.cwd.as_deref() {
            t.cwd = c.to_string();
        }
        if t.session_id.is_empty() {
            if let Some(s) = e.session_id.as_deref() {
                t.session_id = s.to_string();
            }
        }
        if !e.timestamp.is_empty() {
            t.last_entry_ts = e.timestamp.clone();
        }
        match e.kind.as_str() {
            "assistant" => {
                let Some(msg) = e.message.as_ref() else { continue };
                t.tool_calls_count += count_blocks(msg.content.as_ref(), "tool_use", false);
                if e.is_sidechain {
                    add_usage(&mut t, msg, &mut seen);
                    continue;
                }
                if let Some(m) = msg.model.as_deref().filter(|m| !m.starts_with('<')) {
                    t.model_id = m.to_string();
                }
                if let Some(level) = e.effort.as_deref() {
                    t.thinking_level = level.to_string();
                }
                if let Some(u) = msg.usage.as_ref() {
                    context = Some(u.input_tokens + u.cache_read_input_tokens + u.cache_creation_input_tokens);
                }
                add_usage(&mut t, msg, &mut seen);
                working = msg.stop_reason.as_deref() != Some("end_turn");
            }
            "user" if !e.is_sidechain => {
                let content = e.message.as_ref().and_then(|m| m.content.as_ref());
                t.tool_errors_count += count_blocks(content, "tool_result", true);
                if !e.is_meta && !e.is_compact_summary && is_prompt(content) {
                    t.turns_count += 1;
                }
                working = true;
            }
            "system" if e.subtype.as_deref() == Some("compact_boundary") => context = None,
            _ => {}
        }
    }

    if t.model_id.is_empty() && t.input_tokens == 0 && t.output_tokens == 0 {
        return None;
    }
    let mut win = context_window_for(PROVIDER, &t.model_id);
    if win == 0 {
        win = context_window_for("", &t.model_id);
    }
    t.context_window = win;
    t.context_tokens = context.unwrap_or(0);
    t.context_unknown = context.is_none();
    t.context_percent = (win > 0 && t.context_tokens > 0)
        .then(|| t.context_tokens as f64 / win as f64 * 100.0);
    t.is_working = working;
    Some(t)
}

/// Adds a response's usage and cost once per `message.id`.
fn add_usage(t: &mut LiveTelemetry, msg: &ClaudeMessage, seen: &mut HashSet<String>) {
    let Some(u) = msg.usage.as_ref() else { return };
    if let Some(id) = msg.id.as_deref() {
        if !seen.insert(id.to_string()) {
            return;
        }
    }
    let usage = Usage {
        input: u.input_tokens,
        output: u.output_tokens,
        cache_read: u.cache_read_input_tokens,
        cache_write: u.cache_creation_input_tokens,
        cache_write_1h: u.cache_creation.as_ref().map_or(0, |c| c.ephemeral_1h_input_tokens),
        total_tokens: 0,
        reasoning: u.output_tokens_details.as_ref().map_or(0, |d| d.thinking_tokens),
        cost: None,
    };
    t.input_tokens += usage.input;
    t.output_tokens += usage.output;
    t.cache_read += usage.cache_read;
    t.cache_write += usage.cache_write;
    t.reasoning_tokens += usage.reasoning;
    let model = msg.model.as_deref().unwrap_or("");
    if let Some(cost) = catalog_cost(PROVIDER, model).or_else(|| catalog_cost("", model)) {
        t.total_cost += cost.compute_total(&usage);
    }
}

/// Counts content blocks of `kind`; with `errors_only`, only `is_error: true`.
fn count_blocks(content: Option<&serde_json::Value>, kind: &str, errors_only: bool) -> u64 {
    let Some(serde_json::Value::Array(blocks)) = content else { return 0 };
    blocks
        .iter()
        .filter(|b| b.get("type").and_then(|v| v.as_str()) == Some(kind))
        .filter(|b| !errors_only || b.get("is_error").and_then(|v| v.as_bool()) == Some(true))
        .count() as u64
}

/// A prompt the person typed: plain text, not only tool results.
fn is_prompt(content: Option<&serde_json::Value>) -> bool {
    match content {
        Some(serde_json::Value::String(s)) => !s.trim().is_empty(),
        Some(serde_json::Value::Array(blocks)) => blocks
            .iter()
            .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("text")),
        _ => false,
    }
}

/// Skills face state for a Claude transcript (pi's replay over its trail).
pub fn parse_skills(path: &Path) -> Option<super::skills::SkillSnapshotFile> {
    let content = fs::read_to_string(path).ok()?;
    Some(super::skills_live::parse_skills_str(&pi_shape::to_pi_lines(&content)))
}

/// MCP face state for a Claude transcript.
pub fn parse_mcp(path: &Path) -> Option<super::mcp_live::McpTelemetry> {
    let content = fs::read_to_string(path).ok()?;
    super::mcp_live::parse_mcp_str(&pi_shape::to_pi_lines(&content))
}

pub mod pi_shape;
pub mod subscription;

#[cfg(test)]
mod tests;
