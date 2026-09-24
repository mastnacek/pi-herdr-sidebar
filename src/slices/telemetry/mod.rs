use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

pub mod mcp_live;
pub mod skills;
pub mod skills_live;

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

/// Pi model catalog entry (from ~/.pi/agent/models.json overrides and
/// models-store.json — the models.dev snapshot pi maintains). Read directly
/// like the pi agent framework does; nothing hardcoded.
#[derive(Deserialize, Clone)]
struct ModelCost {
    #[serde(default)]
    input: f64,
    #[serde(default)]
    output: f64,
    #[serde(rename = "cacheRead", default)]
    cache_read: f64,
    #[serde(rename = "cacheWrite", default)]
    cache_write: f64,
    /// Tiered pricing: full rates once prompt tokens exceed a threshold.
    #[serde(default)]
    tiers: Vec<CostTier>,
}

#[derive(Deserialize, Clone)]
struct CostTier {
    #[serde(rename = "inputTokensAbove", default)]
    input_tokens_above: u64,
    #[serde(default)]
    input: f64,
    #[serde(default)]
    output: f64,
    #[serde(rename = "cacheRead", default)]
    cache_read: f64,
    #[serde(rename = "cacheWrite", default)]
    cache_write: f64,
}

/// USD rates per million tokens.
struct Rates {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
}

impl ModelCost {
    fn rates(&self) -> Rates {
        Rates {
            input: self.input,
            output: self.output,
            cache_read: self.cache_read,
            cache_write: self.cache_write,
        }
    }
}

impl CostTier {
    fn rates(&self) -> Rates {
        Rates {
            input: self.input,
            output: self.output,
            cache_read: self.cache_read,
            cache_write: self.cache_write,
        }
    }
}

impl ModelCost {
    /// Byte-for-byte port of pi-ai `calculateCost` (see
    /// pi-coding-agent dist chunk: `function calculateCost(model,usage)`).
    /// Long-lived (1h) cache writes bill at 2× the input rate.
    fn compute_total(&self, u: &Usage) -> f64 {
        self.select_for(u).total_cost(u)
    }

    /// Select tiered rates by total prompt tokens (input + cacheRead +
    /// cacheWrite), matching pi-ai's threshold walk.
    fn select_for(&self, u: &Usage) -> Rates {
        let input_tokens = u.input + u.cacheRead + u.cacheWrite;
        let mut rates = self.rates();
        let mut matched: Option<u64> = None;
        for tier in &self.tiers {
            if input_tokens > tier.input_tokens_above
                && tier.input_tokens_above > matched.unwrap_or(0)
            {
                rates = tier.rates();
                matched = Some(tier.input_tokens_above);
            }
        }
        rates
    }
}

impl Rates {
    /// USD for one usage entry, per pi-ai `calculateCost` semantics.
    fn total_cost(&self, u: &Usage) -> f64 {
        let long_write = u.cache_write_1h.min(u.cacheWrite);
        let short_write = u.cacheWrite - long_write;
        let cache_write_cost =
            self.cache_write * short_write as f64 + self.input * 2.0 * long_write as f64;
        (self.input * u.input as f64
            + self.output * u.output as f64
            + self.cache_read * u.cacheRead as f64
            + cache_write_cost)
            / 1e6
    }
}

/// One model row from the pi catalog.
#[derive(Deserialize, Clone)]
struct ModelEntry {
    id: String,
    #[serde(default, rename = "contextWindow")]
    context_window: u64,
    #[serde(default)]
    cost: Option<ModelCost>,
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

type Catalog = Vec<(String, Vec<ModelEntry>)>;

/// Process-level catalog cache keyed by both files' mtimes — parse_session
/// runs every refresh tick, and models-store.json is ~400 KB.
static CATALOG: Mutex<Option<(Option<SystemTime>, Option<SystemTime>, Catalog)>> = Mutex::new(None);

fn catalog_mtimes() -> (Option<SystemTime>, Option<SystemTime>) {
    let home = match crate::shared::dirs_home() {
        Some(h) => h,
        None => return (None, None),
    };
    let mtime = |p: PathBuf| fs::metadata(p).and_then(|m| m.modified()).ok();
    (
        mtime(home.join(".pi").join("agent").join("models.json")),
        mtime(home.join(".pi").join("agent").join("models-store.json")),
    )
}

fn load_catalog() -> Catalog {
    let (m1, m2) = catalog_mtimes();
    if let Ok(guard) = CATALOG.lock() {
        if let Some((c1, c2, cat)) = guard.as_ref() {
            if *c1 == m1 && *c2 == m2 {
                return cat.clone();
            }
        }
    }
    let mut cat: Catalog = Vec::new();
    let home = crate::shared::dirs_home();
    if let Some(home) = home {
        // models.json first: user config overrides the fetched store.
        for (path, providers_key) in [
            (home.join(".pi").join("agent").join("models.json"), true),
            (
                home.join(".pi").join("agent").join("models-store.json"),
                false,
            ),
        ] {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let providers = if providers_key {
                serde_json::from_str::<ProvidersFile>(&text).map(|f| f.providers)
            } else {
                serde_json::from_str::<TopLevelFile>(&text).map(|f| f.providers)
            };
            if let Ok(providers) = providers {
                for (name, cfg) in providers {
                    cat.push((name, cfg.models));
                }
            }
        }
    }
    if let Ok(mut guard) = CATALOG.lock() {
        *guard = Some((m1, m2, cat.clone()));
    }
    cat
}

/// Resolve (provider, model_id) against the pi catalogs. `provider` empty
/// means scan every provider (models.json overrides come first, so an exact
/// user entry wins over the fetched store).
fn catalog_lookup(provider: &str, model_id: &str) -> Option<ModelEntry> {
    let cat = load_catalog();
    let mut any_match: Option<&ModelEntry> = None;
    for (pname, models) in &cat {
        if !provider.is_empty() && pname != provider {
            continue;
        }
        if let Some(m) = models.iter().find(|m| m.id == model_id) {
            if pname == provider {
                return Some(m.clone());
            }
            any_match = any_match.or(Some(m));
        }
    }
    any_match.cloned()
}

fn context_window_for(provider: &str, model_id: &str) -> u64 {
    catalog_lookup(provider, model_id)
        .map(|m| m.context_window)
        .unwrap_or(0)
}

/// pi-catalog cost rates for a model; None when the model has no pricing
/// (local ollama models) or the model is unknown.
fn catalog_cost(provider: &str, model_id: &str) -> Option<ModelCost> {
    catalog_lookup(provider, model_id).and_then(|m| m.cost)
}

/// Add one usage entry's cost to the running total. Uses the JSONL-reported
/// cost when the session provides it (pi computes it with the same formula);
/// otherwise estimates from the pi catalog rates.
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
    /// Long-lived (1h) cache writes — billed at 2× input per pi-ai.
    #[serde(default, rename = "cacheWrite1h")]
    cache_write_1h: u64,
    #[serde(default)]
    totalTokens: u64,
    #[serde(default)]
    reasoning: u64,
    #[serde(default)]
    cost: Option<UsageCost>,
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

    // Cross-project scan below: only use where a wrong-session display is
    // acceptable (interactive pickers), never for tab-scoped sidebars.
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

/// Newest session JSONL strictly inside the cwd-scoped project folder —
/// never scans across projects. Use when displaying tab-scoped telemetry:
/// with multiple agents in a monorepo the cross-project "newest anywhere"
/// pick can be another agent's session.
pub fn find_newest_session_scoped(cwd: Option<&str>) -> Option<PathBuf> {
    let cwd = cwd?;
    let base = crate::shared::dirs_home()?
        .join(".pi")
        .join("agent")
        .join("sessions");
    let slug = session_dir_slug(cwd);
    newest_session_in(&base.join(&slug), "")
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

fn git_info(cwd: &Path) -> Option<GitTelemetry> {
    use std::process::Command;
    // 1. Status with branch info in porcelain format
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["status", "--porcelain=v2", "--branch"])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let status_str = String::from_utf8_lossy(&out.stdout);
    let mut branch = String::new();
    let mut ahead = 0u32;
    let mut behind = 0u32;
    let mut staged = 0u32;
    let mut unstaged = 0u32;
    let mut untracked = 0u32;

    for line in status_str.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            branch = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            // e.g. "+5 -0"
            let mut parts = rest.split_whitespace();
            if let Some(p) = parts.next() {
                if let Some(n) = p.strip_prefix('+').and_then(|s| s.parse::<u32>().ok()) {
                    ahead = n;
                }
            }
            if let Some(p) = parts.next() {
                if let Some(n) = p.strip_prefix('-').and_then(|s| s.parse::<u32>().ok()) {
                    behind = n;
                }
            }
        } else if line.starts_with("1 ") || line.starts_with("2 ") {
            // 1 <XY> ... or 2 <XY> ...
            let bytes = line.as_bytes();
            if bytes.len() >= 4 {
                let x = bytes[2];
                let y = bytes[3];
                if x != b'.' {
                    staged += 1;
                }
                if y != b'.' {
                    unstaged += 1;
                }
            }
        } else if line.starts_with("? ") {
            untracked += 1;
        } else if line.starts_with("u ") {
            // unmerged
            staged += 1;
            unstaged += 1;
        }
    }

    // 2. Latest commit details
    let log_out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["log", "-1", "--format=%h|%cr|%s"])
        .output()
        .ok();

    let mut commit_hash = String::new();
    let mut commit_age = String::new();
    let mut commit_msg = String::new();

    if let Some(lo) = log_out {
        if lo.status.success() {
            let log_line = String::from_utf8_lossy(&lo.stdout);
            let parts: Vec<&str> = log_line.trim().splitn(3, '|').collect();
            if parts.len() >= 3 {
                commit_hash = parts[0].to_string();
                commit_age = parts[1].to_string();
                commit_msg = parts[2].to_string();
            }
        }
    }

    Some(GitTelemetry {
        branch,
        ahead,
        behind,
        staged,
        unstaged,
        untracked,
        commit_hash,
        commit_msg,
        commit_age,
    })
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
    let mut in_turn = false;

    // Parse message counts and tool usage
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
                    if msg.role.as_deref() == Some("user") {
                        t.turns_count += 1;
                        in_turn = true;
                    }
                    if msg.role.as_deref() == Some("toolResult") {
                        in_turn = true;
                        if msg.is_error.unwrap_or(false) {
                            t.tool_errors_count += 1;
                        }
                    }
                    if msg.role.as_deref() == Some("assistant") {
                        let mut has_tool_calls = false;
                        if let Some(serde_json::Value::Array(blocks)) = &msg.content {
                            for b in blocks {
                                if b.get("type").and_then(|v| v.as_str()) == Some("toolCall") {
                                    t.tool_calls_count += 1;
                                    has_tool_calls = true;
                                }
                            }
                        }
                        if has_tool_calls {
                            in_turn = true;
                        } else {
                            // Settled turn ends with plain assistant message
                            in_turn = false;
                        }
                    }

                    if let Some(u) = &msg.usage {
                        // Model identity BEFORE cost math: assistant messages
                        // carry it directly; earlier entries inherit the
                        // session's last known model.
                        let prov = if msg.role.as_deref() == Some("assistant") {
                            msg.provider
                                .as_deref()
                                .filter(|s| !s.is_empty())
                                .or(Some(last_model_provider.as_str()))
                        } else {
                            Some(last_model_provider.as_str())
                        };
                        let mid = if msg.role.as_deref() == Some("assistant") {
                            msg.model
                                .as_deref()
                                .filter(|s| !s.is_empty())
                                .or(Some(last_model_id.as_str()))
                        } else {
                            Some(last_model_id.as_str())
                        };
                        t.input_tokens += u.input;
                        t.output_tokens += u.output;
                        t.cache_read += u.cacheRead;
                        t.cache_write += u.cacheWrite;
                        t.reasoning_tokens += u.reasoning;
                        add_usage_cost(&mut t, u, prov, mid);
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

/// Formula tests for the pi-ai calculateCost port. The reference
/// implementation lives in pi-coding-agent's bundled pi-ai
/// (`function calculateCost(model,usage)`); these keep the port honest.
#[cfg(test)]
mod cost_tests {
    use super::*;

    fn usage(
        input: u64,
        output: u64,
        cache_read: u64,
        cache_write: u64,
        cache_write_1h: u64,
    ) -> Usage {
        Usage {
            input,
            output,
            cacheRead: cache_read,
            cacheWrite: cache_write,
            cache_write_1h,
            totalTokens: 0,
            reasoning: 0,
            cost: None,
        }
    }

    #[test]
    fn flat_rates_match_pi_formula() {
        // rates: $3/M in, $15/M out, $0.30/M cacheRead, $3.75/M cacheWrite
        let cost = ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
            tiers: vec![],
        };
        let u = usage(1_000_000, 100_000, 500_000, 0, 0);
        // pi: 3*1 + 15*0.1 + 0.3*0.5 = 3 + 1.5 + 0.15 = 4.65
        assert!((cost.compute_total(&u) - 4.65).abs() < 1e-9);
    }

    #[test]
    fn one_hour_cache_write_bills_at_2x_input() {
        let cost = ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
            tiers: vec![],
        };
        // 200k short (5-min) + 100k long (1h) cache write
        let u = usage(0, 0, 0, 300_000, 100_000);
        // pi: (3.75*200k + 3*2*100k)/1e6 = (0.75 + 0.6) = 1.35
        assert!((cost.compute_total(&u) - 1.35).abs() < 1e-9);
    }

    #[test]
    fn tiered_rates_pick_highest_matching_threshold() {
        let cost = ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
            tiers: vec![CostTier {
                input_tokens_above: 200_000,
                input: 6.0,
                output: 22.5,
                cache_read: 0.6,
                cache_write: 7.5,
            }],
        };
        let below = usage(100_000, 0, 0, 0, 0);
        // 100k ≤ 200k threshold → base rates 3/M → 0.3
        assert!((cost.compute_total(&below) - 0.3).abs() < 1e-9);
        let above = usage(250_000, 0, 0, 0, 0);
        // 250k > 200k threshold → 6/M → 1.5
        assert!((cost.compute_total(&above) - 1.5).abs() < 1e-9);
    }
}
