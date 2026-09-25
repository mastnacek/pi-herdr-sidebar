//! Session-log discovery, change fingerprinting, on-disk caching and the pass
//! that turns a set of `.jsonl` files into one [`UsageStats`] aggregate.
use super::model::{Counted, PluginUsage, UsageStats};
use super::parse::{scan_file, Counters};
use super::ScanProgress;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Fingerprint of the log set: if it is unchanged, a cached aggregate is still
/// valid and no file has to be read.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fingerprint {
    pub files: usize,
    pub bytes: u64,
    pub newest_mtime: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    fingerprint: Fingerprint,
    stats: UsageStats,
}

/// `~/.pi/agent/sessions` — where pi writes its own session logs.
pub fn sessions_dir() -> Option<PathBuf> {
    Some(
        crate::shared::dirs_home()?
            .join(".pi")
            .join("agent")
            .join("sessions"),
    )
}

/// Recursively collects `.jsonl` files (sessions nest up to three levels deep).
fn collect_sessions(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_sessions(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "jsonl") {
            out.push(path);
        }
    }
}

pub fn session_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(dir) = sessions_dir() {
        collect_sessions(&dir, &mut files);
    }
    files.sort();
    files
}

pub fn fingerprint_of(files: &[PathBuf]) -> Fingerprint {
    let mut fp = Fingerprint {
        files: files.len(),
        ..Default::default()
    };
    for path in files {
        if let Ok(meta) = fs::metadata(path) {
            fp.bytes += meta.len();
            if let Ok(modified) = meta.modified() {
                fp.newest_mtime = fp.newest_mtime.max(mtime_secs(modified));
            }
        }
    }
    fp
}

fn mtime_secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_path() -> PathBuf {
    crate::shared::pi_sidebar_snapshots_dir().join("usage-stats.json")
}

/// Loads the cached aggregate when the log set is unchanged.
pub fn load_cached(fingerprint: &Fingerprint) -> Option<UsageStats> {
    let text = fs::read_to_string(cache_path()).ok()?;
    let cache: CacheFile = serde_json::from_str(&text).ok()?;
    (cache.fingerprint == *fingerprint).then_some(cache.stats)
}

/// Persists the aggregate so the next sidebar start is instant.
pub fn store_cached(fingerprint: &Fingerprint, stats: &UsageStats) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let cache = CacheFile {
        fingerprint: fingerprint.clone(),
        stats: stats.clone(),
    };
    if let Ok(json) = serde_json::to_string(&cache) {
        let _ = fs::write(path, json);
    }
}

/// Blocking scan of every session file, reporting progress as it goes.
///
/// Callers should use the background worker in [`super::spawn_scan`] instead of
/// running this on the UI thread.
pub fn scan_blocking(progress: &ScanProgress) -> Result<UsageStats, String> {
    let started = Instant::now();
    let files = session_files();
    progress.total.store(files.len(), Ordering::Relaxed);

    let mut counters = Counters::default();
    let mut newest = None;
    for path in &files {
        if let Ok(meta) = fs::metadata(path) {
            if let Ok(modified) = meta.modified() {
                let secs = mtime_secs(modified);
                newest = Some(newest.map_or(secs, |n: u64| n.max(secs)));
            }
        }
        scan_file(path, &mut counters);
        progress.done.fetch_add(1, Ordering::Relaxed);
    }
    progress.finish();

    let tool_calls: u64 = counters.tools.values().sum();
    let stats = UsageStats {
        files: files.len(),
        messages: counters.messages,
        tool_calls,
        builtin_calls: counters.builtin_calls(),
        plugins: rank_plugins(&counters.tools),
        tools: top(&counters.tools, usize::MAX),
        skills: top(&counters.skills, usize::MAX),
        commands: top(&counters.commands, 20),
        elapsed_ms: started.elapsed().as_millis() as u64,
        newest_session: newest,
    };
    Ok(stats)
}

fn rank_plugins(tools: &HashMap<String, u64>) -> Vec<PluginUsage> {
    let mut by_plugin: HashMap<&'static str, Vec<(String, u64)>> = HashMap::new();
    for (tool, count) in tools {
        by_plugin
            .entry(super::model::plugin_of(tool))
            .or_default()
            .push((tool.clone(), *count));
    }

    let mut plugins: Vec<PluginUsage> = by_plugin
        .into_iter()
        .map(|(plugin, mut tools)| {
            tools.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let calls = tools.iter().map(|(_, c)| *c).sum();
            PluginUsage {
                plugin: plugin.to_string(),
                calls,
                tools,
            }
        })
        .collect();
    plugins.sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.plugin.cmp(&b.plugin)));
    plugins
}

fn top(map: &HashMap<String, u64>, limit: usize) -> Vec<Counted> {
    let mut v: Vec<Counted> = map
        .iter()
        .map(|(name, count)| Counted {
            name: name.clone(),
            count: *count,
        })
        .collect();
    v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    v.truncate(limit);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_plugins_by_calls_and_keeps_tool_breakdown() {
        let mut tools = HashMap::new();
        tools.insert("ctx_execute".to_string(), 5u64);
        tools.insert("ctx_search".to_string(), 2u64);
        tools.insert("bash".to_string(), 9u64);
        let ranked = rank_plugins(&tools);
        assert_eq!(ranked[0].plugin, "builtin");
        assert_eq!(ranked[0].calls, 9);
        assert_eq!(ranked[1].plugin, "context-mode");
        assert_eq!(ranked[1].calls, 7);
        assert_eq!(ranked[1].tools[0], ("ctx_execute".to_string(), 5));
    }

    #[test]
    fn top_sorts_desc_and_truncates() {
        let mut map = HashMap::new();
        for (name, count) in [("a", 1u64), ("b", 5), ("c", 3)] {
            map.insert(name.to_string(), count);
        }
        let listed = top(&map, 2);
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].name, "b");
        assert_eq!(listed[1].name, "c");
    }

    #[test]
    fn fingerprint_changes_when_a_file_grows() {
        let dir = std::env::temp_dir().join(format!("usage_fp_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.jsonl");
        fs::write(&file, "x").unwrap();
        let files = vec![file.clone()];
        let before = fingerprint_of(&files);
        fs::write(&file, "xxxx").unwrap();
        let after = fingerprint_of(&files);
        assert_ne!(before, after, "byte growth must invalidate the cache");
        assert_eq!(after.files, 1);
        fs::remove_dir_all(&dir).ok();
    }

    /// Walks the real log set and prints the aggregate, so the scanner can be
    /// checked against an independent one-off analysis of the same logs.
    ///
    /// `cargo test --release scans_real_logs -- --ignored --nocapture`
    #[test]
    #[ignore = "reads the real ~/.pi/agent/sessions tree (~300 MB)"]
    fn scans_real_logs_for_manual_verification() {
        let progress = crate::slices::shortcuts::usage::ScanProgress::default();
        let stats = scan_blocking(&progress).expect("scan failed");
        println!(
            "\nfiles={} messages={} tool_calls={} builtin={} elapsed={} ms",
            stats.files, stats.messages, stats.tool_calls, stats.builtin_calls, stats.elapsed_ms
        );
        println!("--- plugins (tool calls):");
        for p in stats.ranked_plugins().iter().take(12) {
            println!("  {:28} {}", p.plugin, p.calls);
        }
        println!("--- skills:");
        for s in stats.skills.iter().take(8) {
            println!("  {:28} {}", s.name, s.count);
        }
        println!("--- commands:");
        for c in stats.commands.iter().take(8) {
            println!("  {:28} {}", c.name, c.count);
        }
    }
}
