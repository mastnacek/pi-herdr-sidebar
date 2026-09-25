//! Herdr action handlers that open this plugin's own panes.
//!
//! Each action maps to a pane entrypoint declared in `herdr-plugin.toml`: Herdr
//! owns the keybinding, and the action merely asks Herdr to open the pane. The
//! entrypoint ids are platform-suffixed (`-win` on Windows), which is why every
//! action handler looks the id up through `cfg!(windows)`.
use crate::shared::HerdrClient;
use std::process::Command;

/// Opens a plugin pane entrypoint defined in `herdr-plugin.toml`.
fn open_entrypoint(entrypoint: &str, label: &str) -> Result<(), String> {
    let client = HerdrClient::new();

    let status = Command::new(&client.bin_path)
        .args([
            "plugin",
            "pane",
            "open",
            "--plugin",
            "pi.herdr-sidebar",
            "--entrypoint",
            entrypoint,
        ])
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("Failed to open {label}"))
    }
}

pub fn run_popup() -> Result<(), String> {
    let entrypoint = if cfg!(windows) {
        "sidebar-popup-win"
    } else {
        "sidebar-popup"
    };
    open_entrypoint(entrypoint, "sidebar popup")
}

/// Opens the SPAI Notes face as a large modal over the active workspace —
/// same mechanic the Kanban board uses (`placement = "popup"`).
pub fn run_notes_popup() -> Result<(), String> {
    let entrypoint = if cfg!(windows) {
        "notes-popup-win"
    } else {
        "notes-popup"
    };
    open_entrypoint(entrypoint, "notes modal")
}

/// Opens the standalone plugin/skill usage overview window — same pane mechanic
/// as the Kanban board (`placement = "popup"`), but with no sidebar chrome.
pub fn run_usage_popup() -> Result<(), String> {
    let entrypoint = if cfg!(windows) { "usage-win" } else { "usage" };
    open_entrypoint(entrypoint, "usage overview")
}
