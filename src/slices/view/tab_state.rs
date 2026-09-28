//! Persisted face selection.
//!
//! A panel pane is recreated on every plugin rebuild/reload (and herdr's
//! `ensure` action reopens it on tab focus), and `SidebarState` used to boot on
//! `Tab::Zen` every time — so a Status monitor had to press `1` after each
//! restart. The face now round-trips through a small state file instead.
//!
//! Location follows the same rule as the weather cache: `HERDR_PLUGIN_STATE_DIR`
//! when herdr injects it, `~/.pi/agent/pi-sidebar/` otherwise.

use std::path::{Path, PathBuf};

use super::state_model::Tab;

/// File name inside the state directory.
const FILE_NAME: &str = "pi-sidebar-tab";

/// Face used when nothing was stored yet, or when the stored value is unusable.
/// Status, not Zen: this panel exists to be monitored.
pub const DEFAULT: Tab = Tab::Status;

/// Stable name for the store — never the enum's index, so reordering faces
/// cannot silently change what a persisted value means.
pub fn slug(tab: Tab) -> &'static str {
    match tab {
        Tab::Zen => "zen",
        Tab::Status => "status",
        Tab::Skills => "skills",
        Tab::Mcp => "mcp",
        Tab::Notes => "notes",
        Tab::Shortcuts => "shortcuts",
    }
}

/// Inverse of [`slug`]. Unknown names are rejected rather than guessed.
pub fn from_slug(value: &str) -> Option<Tab> {
    match value.trim().to_ascii_lowercase().as_str() {
        "zen" => Some(Tab::Zen),
        "status" => Some(Tab::Status),
        "skills" => Some(Tab::Skills),
        "mcp" => Some(Tab::Mcp),
        "notes" => Some(Tab::Notes),
        "shortcuts" => Some(Tab::Shortcuts),
        _ => None,
    }
}

/// Where the face is stored. `state_dir` comes from `PluginContext`; without it
/// the injected env var is used, then the pane-state directory.
fn store_path(state_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(dir) = state_dir {
        return Some(dir.join(FILE_NAME));
    }
    if let Some(dir) = std::env::var_os("HERDR_PLUGIN_STATE_DIR") {
        return Some(PathBuf::from(dir).join(FILE_NAME));
    }
    let home = crate::shared::dirs_home()?;
    Some(
        home.join(".pi")
            .join("agent")
            .join("pi-sidebar")
            .join(FILE_NAME),
    )
}

/// The face to boot on. Missing, unreadable or unknown state all fall back to
/// [`DEFAULT`] — a broken store must never keep the panel off the screen.
pub fn load(state_dir: Option<&Path>) -> Tab {
    let Some(path) = store_path(state_dir) else {
        return DEFAULT;
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return DEFAULT;
    };
    from_slug(&raw).unwrap_or(DEFAULT)
}

/// Remember the face. Best-effort: a read-only or missing state directory must
/// not disturb the running panel.
pub fn save(state_dir: Option<&Path>, tab: Tab) {
    let Some(path) = store_path(state_dir) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, slug(tab));
}

#[cfg(test)]
mod tests;
