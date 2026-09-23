use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub mod skills;
pub mod skills_live;

/// Live telemetry parsed directly from the Pi session JSONL file.
/// No TS extension required — the session log IS the source of truth.
#[derive(Debug, Clone, Default)]
pub struct LiveTelemetry {
    pub session_id: String,
    /// Working directory from the session header (git + display).
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
    pub git_branch: String,
    pub git_dirty: u32,
    pub last_entry_ts: String,
}

#[derive(Deserialize)]
struct SessionHeader {
    cwd: Option<String>,
    id: Option<String>,
}

#[derive(Deserialize)]
struct ModelChange {
    provider: Option<String>,
    #[serde(rename = "modelId")]
    model_id: Option<String>,
}

#[derive(Deserialize)]
struct ThinkingChange {
    #[serde(rename = "thinkingLevel")]
    level: Option<String>,
}

#[derive(Deserialize)]
struct UsageCost {
    #[serde(default)]
    total: f64,
}

#[derive(Deserialize)]
struct Usage {
    #[serde(default)]
    input: u64,
    #[serde(default)]
    output: u64,
    #[serde(default)]
    cacheRead: u64,
    #[serde(default)]
    cacheWrite: u64,
    #[serde(default)]
    totalTokens: u64,
    #[serde(default)]
    cost: Option<UsageCost>,
}

#[derive(Deserialize)]
struct MessageBody {
    role: Option<String>,
    provider: Option<String>,
    model: Option<String>,
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

/// Slug a cwd the same way pi names session folders:
/// `D:\01_programovani\herdr\plugins` -> `--D--01_programovani-herdr-plugins--`
fn session_dir_slug(cwd: &str) -> String {
    let mut slug = String::from("--");
    for c in cwd.chars() {
        if c == ':' || c == '\\' || c == '/' {
            slug.push('-');
        } else {
            slug.push(c);
        }
    }
    slug.push_str("--");
    slug
}

/// Find the newest session file for `session_id` under the pi sessions tree,
/// preferring the folder matching the pane's cwd.
pub fn find_session_file(session_id: &str, cwd: &str) -> Option<PathBuf> {
    let base = crate::shared::dirs_home()?
        .join(".pi")
        .join("agent")
        .join("sessions");
    if session_id.is_empty() {
        return None;
    }

    // Preferred: cwd-scoped folder
    let slug = session_dir_slug(cwd);
    let scoped = base.join(&slug);
    if let Some(hit) = newest_session_in(&scoped, session_id) {
        return Some(hit);
    }

    // Fallback: scan all workspace folders
    for dir in fs::read_dir(&base).ok()?.flatten() {
        if dir.path().is_dir() {
            if let Some(hit) = newest_session_in(&dir.path(), session_id) {
                return Some(hit);
            }
        }
    }
    None
}

fn newest_session_in(dir: &Path, session_id: &str) -> Option<PathBuf> {
    let mut best: Option<(PathBuf, SystemTime)> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".jsonl") && name.contains(session_id) {
            if let Ok(meta) = entry.metadata() {
                let mtime = meta.modified().ok()?;
                match &best {
                    None => best = Some((entry.path(), mtime)),
                    Some((_, t)) if mtime > *t => best = Some((entry.path(), mtime)),
                    _ => {}
                }
            }
        }
    }
    best.map(|(p, _)| p)
}

/// Look up the context window for a model. Sources, in order:
/// 1. `~/.pi/agent/models.json` — user overrides (`providers.<p>.models[]`)
/// 2. `~/.pi/agent/models-store.json` — model catalog (`<p>.models[]`)
fn context_window_for(provider: &str, model_id: &str) -> u64 {
    let Some(home) = crate::shared::dirs_home() else {
        return 0;
    };

    let mut win = lookup_context_window(
        &home.join(".pi").join("agent").join("models.json"),
        Some("providers"),
        provider,
        model_id,
    );
    if win == 0 {
        win = lookup_context_window(
            &home.join(".pi").join("agent").join("models-store.json"),
            None,
            provider,
            model_id,
        );
    }
    win
}

/// Scan one model catalog file for `model_id`'s context window.
///
/// `providers_key` wraps the map when the file nests providers under a key
/// (models.json); `None` reads providers from the top level (models-store.json).
fn lookup_context_window(
    path: &Path,
    providers_key: Option<&str>,
    provider: &str,
    model_id: &str,
) -> u64 {
    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
        #[serde(default, rename = "contextWindow")]
        context_window: u64,
    }
    #[derive(Deserialize)]
    struct ProviderCfg {
        #[serde(default)]
        models: Vec<ModelEntry>,
    }
    #[derive(Deserialize)]
    struct ProvidersFile {
        #[serde(default)]
        providers: std::collections::BTreeMap<String, ProviderCfg>,
    }
    #[derive(Deserialize)]
    struct TopLevelFile {
        #[serde(flatten)]
        providers: std::collections::BTreeMap<String, ProviderCfg>,
    }

    let Ok(text) = fs::read_to_string(path) else {
        return 0;
    };

    let providers: std::collections::BTreeMap<String, ProviderCfg> = if providers_key.is_some() {
        match serde_json::from_str::<ProvidersFile>(&text) {
            Ok(f) => f.providers,
            Err(_) => return 0,
        }
    } else {
        match serde_json::from_str::<TopLevelFile>(&text) {
            Ok(f) => f.providers,
            Err(_) => return 0,
        }
    };

    for (pname, cfg) in &providers {
        if !provider.is_empty() && pname != provider {
            continue;
        }
        for m in &cfg.models {
            if m.id == model_id {
                return m.context_window;
            }
        }
    }
    0
}

/// Newest session JSONL in the pi sessions tree, preferring the folder that
/// matches `cwd`'s slug. Fully independent of herdr's `agentSession` reporting.
pub fn find_newest_session(cwd: Option<&str>) -> Option<PathBuf> {
    let base = crate::shared::dirs_home()?
        .join(".pi")
        .join("agent")
        .join("sessions");

    if let Some(cwd) = cwd {
        let slug = session_dir_slug(cwd);
        let scoped = base.join(&slug);
        if let Some(hit) = newest_session_in(&scoped, "") {
            return Some(hit);
        }
    }

    let mut best: Option<(PathBuf, SystemTime)> = None;
    for dir in fs::read_dir(&base).ok()?.flatten() {
        if dir.path().is_dir() {
            if let Some(hit) = newest_session_in(&dir.path(), "") {
                if let Ok(meta) = fs::metadata(&hit) {
                    if let Ok(mtime) = meta.modified() {
                        match &best {
                            None => best = Some((hit, mtime)),
                            Some((_, t)) if mtime > *t => best = Some((hit, mtime)),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    best.map(|(p, _)| p)
}

/// Load live telemetry for a Pi pane (session id + cwd from `herdr pane list`).
pub fn load(session_id: &str, cwd: &str) -> Option<LiveTelemetry> {
    let session_file = find_session_file(session_id, cwd)?;
    parse_session(&session_file, session_id)
}

/// Load telemetry from the newest session JSONL without knowing a session id.
pub fn load_newest(cwd: Option<&str>) -> Option<LiveTelemetry> {
    let file = find_newest_session(cwd)?;
    parse_session(&file, "")
}

fn git_info(cwd: &Path) -> (String, u32) {
    use std::process::Command;
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output();
    let branch = out
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let dirty = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().count() as u32)
        .unwrap_or(0);

    (branch_display(&branch, &cwd_display(cwd)), dirty)
}

fn branch_display(branch: &str, _cwd: &str) -> String {
    branch.to_string()
}

fn cwd_display(cwd: &Path) -> String {
    cwd.display().to_string()
}

/// Parse a pi session JSONL into telemetry. Tolerant: partial/unknown entries skipped.
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

    for line in content.lines() {
        let Ok(entry) = serde_json::from_str::<Entry>(line) else {
            continue;
        };
        match entry.kind.as_str() {
            "session" => {
                // Header carries the workspace cwd; capture for git + display.
                if let Some(cwd) = &entry.cwd {
                    t.cwd = cwd.clone();
                }
                if t.session_id.is_empty() {
                    if let Some(id) = &entry.session_id {
                        t.session_id = id.clone();
                    }
                }
            }
            "model_change" => {
                if let Some(p) = &entry.provider {
                    last_model_provider = p.clone();
                }
                if let Some(m) = &entry.model_id {
                    last_model_id = m.clone();
                }
            }
            "thinking_level_change" => {
                if let Some(l) = &entry.level {
                    last_thinking = l.clone();
                }
            }
            "message" => {
                if let Some(msg) = &entry.message {
                    if let Some(u) = &msg.usage {
                        t.input_tokens += u.input;
                        t.output_tokens += u.output;
                        t.cache_read += u.cacheRead;
                        t.cache_write += u.cacheWrite;
                        if let Some(c) = &u.cost {
                            t.total_cost += c.total;
                        }
                        // Assistant usage reflects the full prompt size -> context
                        if msg.role.as_deref() == Some("assistant") && u.totalTokens > 0 {
                            t.context_tokens = u.totalTokens;
                        }
                        // Assistant messages carry the active model directly.
                        if msg.role.as_deref() == Some("assistant") {
                            if let Some(p) = &msg.provider {
                                last_model_provider = p.clone();
                            }
                            if let Some(m) = &msg.model {
                                last_model_id = m.clone();
                            }
                        }
                    }
                }
            }
            _ => {
                // branch_summary / compaction usage
                if let Some(u) = &entry.usage {
                    t.input_tokens += u.input;
                    t.output_tokens += u.output;
                    t.cache_read += u.cacheRead;
                    t.cache_write += u.cacheWrite;
                    if let Some(c) = &u.cost {
                        t.total_cost += c.total;
                    }
                }
            }
        }
        if !entry.timestamp.is_empty() {
            t.last_entry_ts = entry.timestamp;
        }
    }

    if t.input_tokens == 0 && t.output_tokens == 0 && last_model_id.is_empty() {
        return None; // nothing usable
    }

    t.provider = last_model_provider;
    t.model_id = last_model_id;
    t.thinking_level = last_thinking;

    // Resolve context window: exact model match first, then any provider match.
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

    // Git info from the session cwd (best effort)
    if let Some(header_line) = content.lines().next() {
        if let Ok(header) = serde_json::from_str::<SessionHeader>(header_line) {
            if let Some(cwd) = header.cwd {
                let (branch, dirty) = git_info(Path::new(&cwd));
                t.git_branch = branch;
                t.git_dirty = dirty;
            }
        }
    }

    Some(t)
}

/// Compact token formatting (k/M) shared with the UI face.
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

/// Cost formatting with statusline precision (4dp < 1c, 3dp < $1, else 2dp).
pub fn fmt_cost(v: f64) -> String {
    if v < 0.01 {
        format!("${:.4}", v)
    } else if v < 1.0 {
        format!("${:.3}", v)
    } else {
        format!("${:.2}", v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Manual validation against a real session file:
    /// PI_SIDEBAR_SESSION_FILE=<path> cargo test -- --ignored
    #[test]
    #[ignore]
    fn parses_real_session() {
        let path = std::env::var("PI_SIDEBAR_SESSION_FILE").expect("set env var");
        let t = parse_session(Path::new(&path), "test").expect("parse");
        println!("cwd: {}", t.cwd);
        println!("provider/model: {}/{}", t.provider, t.model_id);
        println!("thinking: {}", t.thinking_level);
        println!(
            "ctx: {:?}/{} ({:?})",
            t.context_tokens, t.context_window, t.context_percent
        );
        println!(
            "cost: {} in:{} out:{} cr:{} cw:{}",
            fmt_cost(t.total_cost),
            t.input_tokens,
            t.output_tokens,
            t.cache_read,
            t.cache_write
        );
        println!("git: {} dirty:{}", t.git_branch, t.git_dirty);
        assert!(t.context_tokens > 0);
        assert!(!t.model_id.is_empty());
        assert!(!t.thinking_level.is_empty(), "thinkingLevel must be parsed");
    }
}
