//! Ignored dev previews — they read the real session and pane-state dirs.
//!
//!   cargo test preview_newest_session -- --ignored --nocapture
//!   cargo test preview_prompt_sidecar   -- --ignored --nocapture

use super::*;

/// Dev preview: `cargo test preview_newest_session -- --ignored --nocapture`
/// prints the tree for the newest session of the current project.
#[test]
#[ignore = "dev preview, reads the real session log"]
fn preview_newest_session() {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let Some(file) = crate::slices::telemetry::find_newest_session_scoped(Some(&cwd)) else {
        println!("no session found for {cwd}");
        return;
    };
    let Ok(content) = std::fs::read_to_string(&file) else {
        return;
    };
    println!("session: {}", file.display());
    let t = tree_from(&content, &cwd);
    for line in flat(&t) {
        println!("{line}");
    }
}

/// Dev preview of the exact provider against a real sidecar:
/// `cargo test preview_prompt_sidecar -- --ignored --nocapture`
///
/// Reads the newest `*.prompt.json` from the pane-state directory through the
/// same `refresh_prompt_sidecar` the Status face uses, so it exercises the
/// wiring and not just the parser.
#[test]
#[ignore = "dev preview, reads the real pane-state directory"]
fn preview_prompt_sidecar() {
    let dir = crate::shared::pi_sidebar_snapshots_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        println!("no pane-state dir at {}", dir.display());
        return;
    };
    let mut newest: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if !path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".prompt.json"))
        {
            continue;
        }
        let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        if newest.as_ref().is_none_or(|(best, _)| mtime > *best) {
            newest = Some((mtime, path));
        }
    }

    let Some((_, path)) = newest else {
        println!("no .prompt.json sidecar in {}", dir.display());
        return;
    };
    // The refresh helper derives the sidecar from a pane snapshot path.
    let snapshot = std::path::PathBuf::from(
        path.to_string_lossy()
            .trim_end_matches(".prompt.json")
            .to_string()
            + ".json",
    );
    let mut mtime = None;
    let mut sidecar = None;
    super::super::super::state_refresh::refresh_prompt_sidecar(
        Some(&snapshot),
        &mut mtime,
        &mut sidecar,
        true,
    );

    println!("sidecar: {}", path.display());
    let Some(sidecar) = sidecar else {
        println!("sidecar ignored (dead or unreadable)");
        return;
    };
    for line in prompt_tree_lines(&LiveTelemetry::default(), Some(&sidecar)) {
        let text: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        println!("{text}");
    }
}
