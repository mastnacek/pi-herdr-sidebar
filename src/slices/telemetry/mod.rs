use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

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
pub struct GitTelemetry {
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub commit_hash: String,
    pub commit_msg: String,
    pub commit_age: String,
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

#[derive(Deserialize)]
struct MessageBody {
    role: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    #[serde(default, rename = "isError")]
    is_error: Option<bool>,
    #[serde(default)]
    content: Option<serde_json::Value>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    timestamp: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default, rename = "sessionId")]
    session_id: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(rename = "modelId", default)]
    model_id: Option<String>,
    #[serde(default, rename = "thinkingLevel")]
    level: Option<String>,
    #[serde(default)]
    message: Option<MessageBody>,
    #[serde(default)]
    usage: Option<Usage>,
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

    for line in content.lines() {
        let Ok(entry) = serde_json::from_str::<Entry>(line) else {
            continue;
        };
        match entry.kind.as_str() {
            "session" => {
                if let Some(s) = entry.session_id.or(entry.id) {
                    if t.session_id.is_empty() {
                        t.session_id = s;
                    }
                }
                if let Some(c) = entry.cwd {
                    t.cwd = c;
                }
            }
            "turn_start" => in_turn = true,
            "turn_end" => in_turn = false,
            "model_change" => {
                if let Some(p) = entry.provider {
                    last_model_provider = p;
                }
                if let Some(m) = entry.model_id {
                    last_model_id = m;
                }
            }
            "thinking_level_change" => {
                if let Some(l) = entry.level {
                    last_thinking = l;
                }
            }
            "message" => {
                if let Some(msg) = entry.message {
                    if msg.role.as_deref() == Some("user") {
                        t.turns_count += 1;
                    }
                    if msg.role.as_deref() == Some("tool_result") {
                        t.tool_calls_count += 1;
                        if msg.is_error.unwrap_or(false) {
                            t.tool_errors_count += 1;
                        }
                    }
                    if let Some(serde_json::Value::Array(blocks)) = msg.content {
                        for b in blocks {
                            if b.get("type").and_then(|v| v.as_str()) == Some("tool_call") {
                                t.tool_calls_count += 1;
                            }
                        }
                    }
                    if let Some(u) = msg.usage {
                        t.input_tokens += u.input;
                        t.output_tokens += u.output;
                        t.cache_read += u.cache_read;
                        t.cache_write += u.cache_write;
                        t.reasoning_tokens += u.reasoning;
                        add_usage_cost(
                            &mut t,
                            &u,
                            msg.provider
                                .as_deref()
                                .or(Some(last_model_provider.as_str())),
                            msg.model.as_deref().or(Some(last_model_id.as_str())),
                        );
                        if msg.role.as_deref() == Some("assistant") && u.total_tokens > 0 {
                            t.context_tokens = u.total_tokens;
                        }
                        if msg.role.as_deref() == Some("assistant") {
                            if let Some(p) = msg.provider {
                                last_model_provider = p;
                            }
                            if let Some(m) = msg.model {
                                last_model_id = m;
                            }
                        }
                    }
                }
            }
            _ => {
                if let Some(u) = entry.usage {
                    t.input_tokens += u.input;
                    t.output_tokens += u.output;
                    t.cache_read += u.cache_read;
                    t.cache_write += u.cache_write;
                    t.reasoning_tokens += u.reasoning;
                    add_usage_cost(
                        &mut t,
                        &u,
                        Some(last_model_provider.as_str()),
                        Some(last_model_id.as_str()),
                    );
                }
            }
        }
        if !entry.timestamp.is_empty() {
            t.last_entry_ts = entry.timestamp;
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
