use crate::shared::HerdrClient;
use std::process::Command;

pub fn run_popup() -> Result<(), String> {
    let client = HerdrClient::new();
    let status = Command::new(&client.bin_path)
        .args([
            "plugin",
            "pane",
            "open",
            "--plugin",
            "pi.sidebar",
            "--entrypoint",
            "sidebar-popup",
        ])
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err("Failed to open sidebar popup".to_string())
    }
}
