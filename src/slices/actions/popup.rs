use crate::shared::HerdrClient;
use std::process::Command;

pub fn run_popup() -> Result<(), String> {
    let client = HerdrClient::new();
    let entrypoint = if cfg!(windows) {
        "sidebar-popup-win"
    } else {
        "sidebar-popup"
    };

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
        Err("Failed to open sidebar popup".to_string())
    }
}
