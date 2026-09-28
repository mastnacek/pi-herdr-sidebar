//! Rendering tests for the Status-face system-prompt tree.

use super::*;
use crate::slices::telemetry::prompt_tree::parse_prompt_tree;
use crate::slices::telemetry::LiveTelemetry;

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
    prompt_tree_lines(text)
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

#[test]
fn tree_always_shows_the_documented_frame_in_order() {
    let log = system(
        r#"{"preamble":"You are an expert coding assistant.","tools":"<tools/>","cwd":"D:/work"}"#,
        r#""toolsAdded":[{"name":"read"}]"#,
    );
    let rendered = joined(&tree_from(&log, "D:/work"));

    let positions: Vec<usize> = super::SECTION_ORDER
        .iter()
        .map(|name| {
            rendered
                .find(name)
                .unwrap_or_else(|| panic!("{name} missing"))
        })
        .collect();
    let mut sorted = positions.clone();
    sorted.sort_unstable();
    assert_eq!(positions, sorted, "sections must render in build order");
}

#[test]
fn missing_addendum_is_visible_as_absent() {
    let log = system(r#"{"preamble":"p","tools":"t","cwd":"D:/work"}"#, "");
    let rendered = joined(&tree_from(&log, "D:/work"));
    assert!(rendered.contains("addendum"));
    assert!(
        rendered.contains("nepřítomno"),
        "the absent addendum is a one-line status:\n{rendered}"
    );
    // No paths of files that were not loaded — that is the noise the panel avoids.
    assert!(!rendered.contains("APPEND_SYSTEM.md"), "{rendered}");
}

#[test]
fn addendum_file_renders_under_the_section() {
    let log = system(
        r#"{"preamble":"p","tools":"t","addendum":"EXTRA","cwd":"D:/work"}"#,
        "",
    );
    let rendered = joined(&tree_from(&log, "D:/work"));
    assert!(
        rendered.contains("inline"),
        "unmatched addendum is inline:\n{rendered}"
    );
}

#[test]
fn agents_files_render_under_project_context() {
    let log = system(
        r#"{"preamble":"p","tools":"t","project_context":"Project-specific instructions and guidelines:\n\n<project_instructions path=\"~/.pi/agent/AGENTS.md\">user</project_instructions>\n\n<project_instructions path=\"D:/work/AGENTS.md\">proj</project_instructions>","cwd":"D:/work"}"#,
        "",
    );
    let rendered = joined(&tree_from(&log, "D:/work"));
    assert!(rendered.contains("AGENTS.md"));
    assert!(rendered.contains("D:/work/AGENTS.md"), "{rendered}");
    assert!(rendered.contains("project_context"));
}

#[test]
fn tools_summary_counts_the_replayed_loadout() {
    let log = system(
        r#"{"preamble":"p","tools":"t","cwd":"D:/work"}"#,
        r#""toolsAdded":[{"name":"read"},{"name":"bash"},{"name":"edit"}]"#,
    );
    let rendered = joined(&tree_from(&log, "D:/work"));
    assert!(rendered.contains("3 nástrojů"), "{rendered}");
    assert!(rendered.contains("read, bash, edit"), "{rendered}");
}

#[test]
fn without_a_prompt_section_the_face_says_so() {
    let rendered = flat(&tree_from("", "D:/work"));
    assert!(rendered[0].contains("Systémový prompt"));
    assert!(rendered[0].contains("systémová zpráva"));
    assert_eq!(rendered.len(), 2, "hint line plus a spacer");
}

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

#[test]
fn files_that_were_not_loaded_are_not_listed() {
    let dir = temp_dir("unloaded");
    let pi_dir = dir.join(".pi");
    std::fs::create_dir_all(&pi_dir).unwrap();
    // On disk, but the prompt never carried it (e.g. untrusted project).
    std::fs::write(pi_dir.join("APPEND_SYSTEM.md"), "NOT LOADED\n").unwrap();

    let log = system(
        r#"{"preamble":"p","tools":"t","cwd":"D:/work"}"#,
        "",
    );
    let rendered = joined(&tree_from(&log, &dir.display().to_string()));
    assert!(!rendered.contains("APPEND_SYSTEM.md"), "{rendered}");
    assert!(!rendered.contains("nepoužito"), "{rendered}");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_drifted_source_is_flagged_for_reload() {
    let dir = temp_dir("driftview");
    let pi_dir = dir.join(".pi");
    std::fs::create_dir_all(&pi_dir).unwrap();
    std::fs::write(pi_dir.join("APPEND_SYSTEM.md"), "LOADED\n").unwrap();

    let log = [
        system_at(
            "2020-01-01T00:00:00.000Z",
            r#"{"preamble":"p","tools":"t"}"#,
            "",
        ),
        system_at("2020-01-01T00:00:01.000Z", r#"{"addendum":"LOADED"}"#, ""),
    ]
    .join("\n");

    let rendered = joined(&tree_from(&log, &dir.display().to_string()));
    assert!(rendered.contains("APPEND_SYSTEM.md"), "{rendered}");
    assert!(rendered.contains("/reload"), "{rendered}");
    assert!(rendered.contains("načteno"), "{rendered}");

    std::fs::remove_dir_all(&dir).ok();
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
