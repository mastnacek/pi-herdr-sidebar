//! Rendering tests for the Status-face system-prompt tree.
//!
//! Split by provider so each file stays small: [`replay`] exercises the
//! transcript path, [`exact`] the `.prompt.json` sidecar, and [`dev`] holds the
//! ignored previews that read real session/pane state.

use super::*;
use crate::slices::telemetry::prompt_tree::parse_prompt_tree;
use crate::slices::telemetry::LiveTelemetry;

mod dev;
mod exact;
mod replay;

fn system(sections: &str, extra: &str) -> String {
    system_at("", sections, extra)
}

/// System message with an explicit transcript timestamp.
fn system_at(ts: &str, sections: &str, extra: &str) -> String {
    let mut message = serde_json::json!({
        "role": "system",
        "content": "",
        "sections": serde_json::from_str::<serde_json::Value>(sections).unwrap_or_default(),
    });
    if !extra.is_empty() {
        let patch: serde_json::Value =
            serde_json::from_str(&format!("{{{extra}}}")).unwrap_or_default();
        if let Some(obj) = patch.as_object() {
            for (key, value) in obj {
                message[key] = value.clone();
            }
        }
    }

    let mut entry = serde_json::json!({ "type": "message", "message": message });
    if !ts.is_empty() {
        entry["timestamp"] = serde_json::json!(ts);
    }
    entry.to_string()
}

fn tree_from(log: &str, cwd: &str) -> LiveTelemetry {
    LiveTelemetry {
        prompt: parse_prompt_tree(log, cwd),
        cwd: cwd.to_string(),
        ..Default::default()
    }
}

fn flat(text: &LiveTelemetry) -> Vec<String> {
    prompt_tree_lines(text, None)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect()
}

fn joined(text: &LiveTelemetry) -> String {
    flat(text).join("\n")
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pi-sidebar-promptview-{}-{}-{tag}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn sidecar(json: &str) -> PromptSidecar {
    serde_json::from_str(json).expect("sidecar json")
}
