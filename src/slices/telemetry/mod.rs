use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub mod context_usage;
pub mod git_live;
pub mod mcp_live;
pub mod model_catalog;
pub mod openrouter_live;
pub mod quota_live;
pub mod session_finder;
pub mod skills;
pub mod skills_live;
pub mod spai_live;
pub mod weather_live;
pub mod yr_api;

use git_live::git_info;
use model_catalog::{catalog_cost, context_window_for};

pub use session_finder::{find_newest_session, find_newest_session_scoped, find_session_file};

#[derive(Debug, Clone, Default)]
pub struct GitCommitLog {
    pub hash: String,
    pub age: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct GitTelemetry {
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub recent_commits: Vec<GitCommitLog>,
    /// Repositories discovered from session edit trail (monorepo mode).
    /// Empty when cwd itself is a repo or no edits yet.
    pub touched_repos: Vec<RepoTelemetry>,
}

/// A nested git repository discovered via the session edit trail.
#[derive(Debug, Clone, Default)]
pub struct RepoTelemetry {
    /// Repo root directory name (e.g. "pi-eval-harness").
    pub name: String,
    /// Full path to repo root.
    pub root: PathBuf,
    /// Number of session-edited files inside this repo.
    pub touched_files: u32,
    /// Recent commits (last 2).
    pub recent_commits: Vec<GitCommitLog>,
}

#[derive(Debug, Clone, Default)]
pub struct LiveTelemetry {
    pub session_id: String,
    pub cwd: String,
    pub session_file: Option<PathBuf>,
    pub provider: String,
    pub model_id: String,
    pub thinking_level: String,
    pub context_tokens: u64,
    pub context_window: u64,
    pub context_percent: Option<f64>,
    /// pi itself reports the context as unknown (post-compaction gap, or no
    /// trustworthy usage yet) — render `?`, not `0.0%`.
    pub context_unknown: bool,
    pub total_cost: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub reasoning_tokens: u64,
    pub turns_count: u64,
    pub tool_calls_count: u64,
    pub tool_errors_count: u64,
    pub git: Option<GitTelemetry>,
    pub git_branch: String,
    pub git_dirty: u32,
    pub is_working: bool,
    pub last_entry_ts: String,
}

#[derive(Deserialize)]
struct SessionHeader {
    cwd: Option<String>,
    id: Option<String>,
}

#[derive(Deserialize)]
pub struct UsageCost {
    #[serde(default)]
    pub total: f64,
}

#[derive(Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input: u64,
    #[serde(default)]
    pub output: u64,
    #[serde(default, rename = "cacheRead")]
    pub cache_read: u64,
    #[serde(default, rename = "cacheWrite")]
    pub cache_write: u64,
    #[serde(default, rename = "cacheWrite1h")]
    pub cache_write_1h: u64,
    #[serde(default, rename = "totalTokens")]
    pub total_tokens: u64,
    #[serde(default)]
    pub reasoning: u64,
    #[serde(default)]
    pub cost: Option<UsageCost>,
}

#[derive(Deserialize, Default)]
pub struct MessageBody {
    pub role: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    #[serde(default, rename = "isError")]
    pub is_error: Option<bool>,
    /// pi marks aborted/errored attempts here; their usage is not context.
    #[serde(default, rename = "stopReason")]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub content: Option<serde_json::Value>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Deserialize, Default)]
pub struct Entry {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(rename = "modelId", default)]
    pub model_id: Option<String>,
    #[serde(default, rename = "thinkingLevel")]
    pub level: Option<String>,
    #[serde(default)]
    pub message: Option<MessageBody>,
    #[serde(default)]
    pub usage: Option<Usage>,
    /// `custom_message` payload (extension-injected context).
    #[serde(default)]
    pub content: Option<serde_json::Value>,
    /// `branch_summary` / `compaction` summary text.
    #[serde(default)]
    pub summary: Option<String>,
    /// `context_edit.targetId` — the entry whose context contribution changes.
    #[serde(default, rename = "targetId")]
    pub target_id: Option<String>,
    /// `context_edit.replacement`; `null`/absent means the target is omitted.
    #[serde(default)]
    pub replacement: Option<serde_json::Value>,
}

pub fn parse_newest_session(cwd: Option<&str>) -> Option<LiveTelemetry> {
    let file = find_newest_session(cwd)?;
    parse_session(&file, "")
}

fn add_usage_cost(t: &mut LiveTelemetry, u: &Usage, provider: Option<&str>, model: Option<&str>) {
    match &u.cost {
        Some(c) => t.total_cost += c.total,
        None => {
            if let Some(cost) = catalog_cost(provider.unwrap_or(""), model.unwrap_or("")) {
                t.total_cost += cost.compute_total(u);
            }
        }
    }
}

pub fn parse_session(path: &Path, session_id: &str) -> Option<LiveTelemetry> {
    let content = fs::read_to_string(path).ok()?;
    let mut t = LiveTelemetry {
        session_id: session_id.to_string(),
        session_file: Some(path.to_path_buf()),
        ..Default::default()
    };

    let mut last_model_provider = String::new();
    let mut last_model_id = String::new();
    let mut last_thinking = String::new();
    let mut in_turn = false;

    // Parsed once so context accounting can look at the whole log (projection
    // rules need entry order: last usage vs latest context_edit/compaction).
    let entries: Vec<Entry> = content
        .lines()
        .filter_map(|line| serde_json::from_str::<Entry>(line).ok())
        .collect();

    for entry in &entries {
        match entry.kind.as_str() {
            "session" => {
                if t.session_id.is_empty() {
                    if let Some(s) = entry.session_id.as_deref().or(entry.id.as_deref()) {
                        t.session_id = s.to_string();
                    }
                }
                if let Some(c) = entry.cwd.as_deref() {
                    t.cwd = c.to_string();
                }
            }
            "turn_start" => in_turn = true,
            "turn_end" => in_turn = false,
            "model_change" => {
                if let Some(p) = entry.provider.as_deref() {
                    last_model_provider = p.to_string();
                }
                if let Some(m) = entry.model_id.as_deref() {
                    last_model_id = m.to_string();
                }
            }
            "thinking_level_change" => {
                if let Some(l) = entry.level.as_deref() {
                    last_thinking = l.to_string();
                }
            }
            "message" => {
                if let Some(msg) = entry.message.as_ref() {
                    if msg.role.as_deref() == Some("user") {
                        t.turns_count += 1;
                    }
                    if msg.role.as_deref() == Some("tool_result") {
                        t.tool_calls_count += 1;
                        if msg.is_error.unwrap_or(false) {
                            t.tool_errors_count += 1;
                        }
                    }
                    if let Some(serde_json::Value::Array(blocks)) = msg.content.as_ref() {
                        for b in blocks {
                            if b.get("type").and_then(|v| v.as_str()) == Some("tool_call") {
                                t.tool_calls_count += 1;
                            }
                        }
                    }
                    if let Some(u) = msg.usage.as_ref() {
                        t.input_tokens += u.input;
                        t.output_tokens += u.output;
                        t.cache_read += u.cache_read;
                        t.cache_write += u.cache_write;
                        t.reasoning_tokens += u.reasoning;
                        add_usage_cost(
                            &mut t,
                            u,
                            msg.provider
                                .as_deref()
                                .or(Some(last_model_provider.as_str())),
                            msg.model.as_deref().or(Some(last_model_id.as_str())),
                        );
                        if msg.role.as_deref() == Some("assistant") {
                            if let Some(p) = msg.provider.as_deref() {
                                last_model_provider = p.to_string();
                            }
                            if let Some(m) = msg.model.as_deref() {
                                last_model_id = m.to_string();
                            }
                        }
                    }
                }
            }
            _ => {
                if let Some(u) = entry.usage.as_ref() {
                    t.input_tokens += u.input;
                    t.output_tokens += u.output;
                    t.cache_read += u.cache_read;
                    t.cache_write += u.cache_write;
                    t.reasoning_tokens += u.reasoning;
                    add_usage_cost(
                        &mut t,
                        u,
                        Some(last_model_provider.as_str()),
                        Some(last_model_id.as_str()),
                    );
                }
            }
        }
        if !entry.timestamp.is_empty() {
            t.last_entry_ts = entry.timestamp.clone();
        }
    }

    if t.input_tokens == 0 && t.output_tokens == 0 && last_model_id.is_empty() {
        return None;
    }

    t.provider = last_model_provider;
    t.model_id = last_model_id;
    t.thinking_level = last_thinking;

    let mut win = context_window_for(&t.provider, &t.model_id);
    if win == 0 {
        win = context_window_for("", &t.model_id);
    }
    t.context_window = win;
    // Same rules pi applies: context_edit/compaction invalidate an older usage.
    let context = context_usage::estimate_context_tokens(&entries);
    t.context_tokens = context.unwrap_or(0);
    t.context_unknown = context.is_none();
    t.context_percent = if win > 0 && t.context_tokens > 0 {
        Some((t.context_tokens as f64 / win as f64) * 100.0)
    } else {
        None
    };

    if let Some(header_line) = content.lines().next() {
        if let Ok(header) = serde_json::from_str::<SessionHeader>(header_line) {
            if let Some(cwd) = header.cwd {
                if let Some(git) = git_info(Path::new(&cwd)) {
                    t.git_branch = git.branch.clone();
                    t.git_dirty = git.staged + git.unstaged + git.untracked;
                    t.git = Some(git);
                }
                // Monorepo mode: cwd itself not a repo → discover nested repos
                // from the session edit trail (edit/write tool call paths).
                let cwd_is_repo = t.git.is_some();
                if !cwd_is_repo {
                    t.git = Some(git_live::build_monorepo_telemetry(
                        Path::new(&cwd),
                        &content,
                    ));
                }
            }
        }
    }

    t.is_working = in_turn;
    Some(t)
}

pub fn fmt_tokens(n: u64) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 10_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else if n < 1_000_000 {
        format!("{}k", n / 1_000)
    } else if n < 10_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else {
        format!("{}M", n / 1_000_000)
    }
}

pub fn fmt_cost(v: f64) -> String {
    if v < 0.01 {
        format!("${:.4}", v)
    } else if v < 1.0 {
        format!("${:.3}", v)
    } else {
        format!("${:.2}", v)
    }
}
