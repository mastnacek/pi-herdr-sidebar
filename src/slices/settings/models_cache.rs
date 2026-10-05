//! Disk caching and HTTP fetch for OpenRouter models.
use super::models::{parse_openrouter_json, ModelInfo};
use std::path::PathBuf;
use std::process::Command;

const FETCH_TIMEOUT_SECS: u32 = 8;

pub fn fetch_openrouter_models() -> Option<Vec<ModelInfo>> {
    let output = Command::new("curl")
        .args([
            "-s",
            "-m",
            &FETCH_TIMEOUT_SECS.to_string(),
            "-A",
            "herdr-pi-sidebar/0.1 github.com/mastnacek/pi-herdr-sidebar",
            "https://openrouter.ai/api/v1/models?output_modalities=all",
        ])
        .output()
        .ok()?;

    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }
    let body = String::from_utf8_lossy(&output.stdout);
    let models = parse_openrouter_json(&body);
    if !models.is_empty() {
        save_cached_models(&models);
        Some(models)
    } else {
        None
    }
}

fn cache_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("HERDR_PLUGIN_STATE_DIR") {
        let p = PathBuf::from(dir).join("openrouter_models.json");
        return Some(p);
    }
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(|h| {
            PathBuf::from(h)
                .join(".pi")
                .join("agent")
                .join("openrouter_models.json")
        })
}

pub fn save_cached_models(models: &[ModelInfo]) {
    let Some(path) = cache_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(models) {
        let _ = std::fs::write(path, json);
    }
}

pub fn load_cached_models() -> Option<Vec<ModelInfo>> {
    let path = cache_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<Vec<ModelInfo>>(&content).ok()
}
