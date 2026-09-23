use std::env;
use std::fs;
use std::path::PathBuf;

/// Injected Herdr environment and runtime context.
#[allow(dead_code)]
pub struct PluginContext {
    pub bin_path: String,
    pub pane_id: Option<String>,
    pub tab_id: Option<String>,
    pub workspace_id: Option<String>,
    pub config_dir: Option<PathBuf>,
    pub state_dir: Option<PathBuf>,
}

impl PluginContext {
    pub fn load() -> Self {
        Self {
            bin_path: env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".to_string()),
            pane_id: env::var("HERDR_PANE_ID").ok(),
            tab_id: env::var("HERDR_TAB_ID").ok(),
            workspace_id: env::var("HERDR_WORKSPACE_ID").ok(),
            config_dir: env::var_os("HERDR_PLUGIN_CONFIG_DIR").map(PathBuf::from),
            state_dir: env::var_os("HERDR_PLUGIN_STATE_DIR").map(PathBuf::from),
        }
    }
}

/// Directory where `pi-sidebar` writes its per-session snapshots.
pub fn pi_sidebar_snapshots_dir() -> PathBuf {
    if let Some(home) = dirs_home() {
        home.join(".pi").join("agent").join("pi-sidebar")
    } else {
        PathBuf::from(".pi-sidebar")
    }
}

/// Cross-platform home directory finder.
pub fn dirs_home() -> Option<PathBuf> {
    if let Ok(userprofile) = env::var("USERPROFILE") {
        return Some(PathBuf::from(userprofile));
    }
    if let Ok(home) = env::var("HOME") {
        return Some(PathBuf::from(home));
    }
    None
}

/// Safe snapshot key derived from pane ID (e.g. `w1M:p22` -> `w1M_p22.json`).
pub fn sanitize_key(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Exact snapshot path for a specific Pi pane ID.
pub fn snapshot_path_for_pane(pane_id: &str) -> PathBuf {
    let safe = sanitize_key(pane_id);
    pi_sidebar_snapshots_dir().join(format!("{}.json", safe))
}

/// Find matching or newest snapshot file.
pub fn find_active_snapshot(target_pane_id: Option<&str>) -> Option<PathBuf> {
    let dir = pi_sidebar_snapshots_dir();
    if !dir.exists() {
        return None;
    }

    // 1. Try exact match if pane_id provided
    if let Some(pane_id) = target_pane_id {
        let safe = sanitize_key(pane_id);
        let candidate = dir.join(format!("{}.json", safe));
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // 2. Pick the most recently modified .json file in the snapshot dir
    let entries = fs::read_dir(&dir).ok()?;
    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            // Exclude request files like *.request.json
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name.ends_with(".request.json") {
                continue;
            }

            if let Ok(metadata) = entry.metadata() {
                if let Ok(mtime) = metadata.modified() {
                    match &newest {
                        None => newest = Some((path, mtime)),
                        Some((_, best_mtime)) => {
                            if mtime > *best_mtime {
                                newest = Some((path, mtime));
                            }
                        }
                    }
                }
            }
        }
    }

    newest.map(|(path, _)| path)
}
