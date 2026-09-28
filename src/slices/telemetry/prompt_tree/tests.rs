//! Unit tests for the system-prompt replay.

use super::*;
use serde_json::{json, Value};

/// Serialize one JSON entry to a session line.
fn line(entry: Value) -> String {
    entry.to_string()
}

fn system(sections: Value, extra: Value) -> String {
    system_at("", sections, extra)
}

/// System message with an explicit transcript timestamp.
fn system_at(ts: &str, sections: Value, extra: Value) -> String {
    let mut message = json!({ "role": "system", "content": "", "sections": sections });
    if let Some(obj) = extra.as_object() {
        for (k, v) in obj {
            message[k] = v.clone();
        }
    }
    let mut entry = json!({ "type": "message", "message": message });
    if !ts.is_empty() {
        entry["timestamp"] = json!(ts);
    }
    line(entry)
}

fn first_request() -> String {
    system(
        json!({
            "preamble": "You are an expert coding assistant.",
            "tools": "<tools>\n- read: reads\n</tools>",
            "rules": "<rules>\n- be concise\n</rules>",
            "docs": "<docs>\n- README\n</docs>",
            "project_context": "Project-specific instructions and guidelines:\n\n<project_instructions path=\"~/.pi/agent/AGENTS.md\">\nuser rules\n</project_instructions>\n\n<project_instructions path=\"D:/work/AGENTS.md\">\nproject rules\n</project_instructions>",
            "cwd": "D:/work",
        }),
        json!({
            "toolsAdded": [{ "name": "read" }, { "name": "bash" }],
        }),
    )
}

#[test]
fn sections_follow_the_documented_build_order() {
    let tree = parse_prompt_tree(&first_request(), "D:/work").expect("tree");
    let names: Vec<&str> = tree.sections.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "preamble",
            "tools",
            "rules",
            "docs",
            "project_context",
            "cwd"
        ]
    );
    assert!(tree.sections.iter().all(|s| !s.custom));
    assert_eq!(
        tree.sections
            .iter()
            .find(|s| s.name == "preamble")
            .unwrap()
            .preview,
        "You are an expert coding assistant."
    );
}

#[test]
fn project_context_lists_every_agents_file_in_render_order() {
    let tree = parse_prompt_tree(&first_request(), "D:/work").expect("tree");
    let paths: Vec<&str> = tree.context_files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, vec!["~/.pi/agent/AGENTS.md", "D:/work/AGENTS.md"]);
    assert!(tree.context_files[0].chars > 0);
}

#[test]
fn later_system_messages_patch_by_name_and_null_removes() {
    let log = [
        first_request(),
        system(json!({ "skills": "<skills>\n- x\n</skills>" }), Value::Null),
        system(json!({ "rules": null }), Value::Null),
    ]
    .join("\n");

    let tree = parse_prompt_tree(&log, "D:/work").expect("tree");
    assert_eq!(tree.patches, 2);

    let skills = tree.sections.iter().find(|s| s.name == "skills").unwrap();
    assert!(skills.patches == 0 && !skills.removed);

    let rules = tree.sections.iter().find(|s| s.name == "rules").unwrap();
    assert!(rules.removed, "null removes a section");
    assert_eq!(rules.chars, 0);
    assert_eq!(rules.patches, 1);
}

#[test]
fn tool_loadout_replays_added_and_removed() {
    let log = [
        first_request(),
        system(Value::Null, json!({ "toolsAdded": [{ "name": "edit" }] })),
        system(Value::Null, json!({ "toolsRemoved": [{ "name": "bash" }] })),
    ]
    .join("\n");

    let tree = parse_prompt_tree(&log, "D:/work").expect("tree");
    assert_eq!(tree.tools, vec!["read", "edit"]);
    assert_eq!(tree.tools_added, 3);
    assert_eq!(tree.tools_removed, 1);
}

#[test]
fn addendum_without_matching_file_is_inline() {
    let log = [
        first_request(),
        system(json!({ "addendum": "APPENDED RULES" }), Value::Null),
    ]
    .join("\n");

    let tree = parse_prompt_tree(&log, "D:/work").expect("tree");
    assert_eq!(
        tree.append_system.map(|(source, _)| source),
        Some(PromptSource::Inline)
    );
    let addendum = tree.sections.iter().find(|s| s.name == "addendum").unwrap();
    assert_eq!(addendum.chars, "APPENDED RULES".len());
}

#[test]
fn addendum_is_attributed_to_the_project_append_system_file() {
    let dir = temp_dir("append");
    let pi_dir = dir.join(".pi");
    std::fs::create_dir_all(&pi_dir).unwrap();
    std::fs::write(pi_dir.join("APPEND_SYSTEM.md"), "PROJECT ADDENDUM\n").unwrap();

    let log = [
        first_request(),
        system(json!({ "addendum": "PROJECT ADDENDUM" }), Value::Null),
    ]
    .join("\n");

    let tree = parse_prompt_tree(&log, &dir.display().to_string()).expect("tree");
    match tree.append_system {
        Some((PromptSource::File(path), _)) => assert!(path.ends_with("APPEND_SYSTEM.md")),
        other => panic!("expected file attribution, got {other:?}"),
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn default_prompt_is_not_an_override_but_missing_tools_is() {
    let with_tools = parse_prompt_tree(&first_request(), "D:/work").unwrap();
    assert!(with_tools.system_override.is_none());

    let log = system(
        json!({ "preamble": "CUSTOM PROMPT", "cwd": "D:/work" }),
        Value::Null,
    );
    let tree = parse_prompt_tree(&log, "D:/work").unwrap();
    assert!(tree.system_override.is_some());
    assert_eq!(
        tree.system_override.map(|(source, _)| source),
        Some(PromptSource::Inline)
    );
}

#[test]
fn extension_sections_render_after_the_builtin_ones() {
    let log = [
        first_request(),
        system(
            json!({ "pi-prompt-translate": "translate please" }),
            Value::Null,
        ),
    ]
    .join("\n");

    let tree = parse_prompt_tree(&log, "D:/work").expect("tree");
    let last = tree.sections.last().unwrap();
    assert_eq!(last.name, "pi-prompt-translate");
    assert!(last.custom);
}

#[test]
fn a_session_without_a_system_message_yields_nothing() {
    let log = line(json!({
        "type": "message",
        "message": { "role": "user", "content": "hi" }
    }));
    assert!(parse_prompt_tree(&log, "D:/work").is_none());
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pi-sidebar-prompt-{}-{}-{tag}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn loaded_at_tracks_the_newest_system_message() {
    let log = [
        system_at(
            "2026-09-28T10:19:22.000Z",
            json!({ "preamble": "p", "tools": "t" }),
            Value::Null,
        ),
        system_at(
            "2026-09-28T11:00:00.000Z",
            json!({ "rules": "r" }),
            Value::Null,
        ),
    ]
    .join("\n");

    let tree = parse_prompt_tree(&log, "D:/work").unwrap();
    assert_eq!(tree.loaded_at, "2026-09-28T11:00:00.000Z");
}

#[test]
fn iso_timestamps_become_epoch_ms() {
    assert_eq!(
        super::sources::iso_to_epoch_ms("1970-01-01T00:00:00.000Z"),
        0
    );
    assert_eq!(
        super::sources::iso_to_epoch_ms("2026-09-28T10:19:22Z"),
        1_790_590_762_000
    );
    assert_eq!(
        super::sources::iso_to_epoch_ms("2026-09-28T10:19:22.950Z"),
        1_790_590_762_950
    );
    assert_eq!(super::sources::iso_to_epoch_ms("garbage"), 0);
    assert_eq!(super::sources::iso_to_epoch_ms(""), 0);
}

#[test]
fn a_source_edited_after_the_prompt_is_flagged_modified() {
    let dir = temp_dir("drift");
    let pi_dir = dir.join(".pi");
    std::fs::create_dir_all(&pi_dir).unwrap();
    std::fs::write(pi_dir.join("APPEND_SYSTEM.md"), "PROJECT ADDENDUM\n").unwrap();

    let log = [
        system_at(
            "2020-01-01T00:00:00.000Z",
            json!({ "preamble": "p", "tools": "t" }),
            Value::Null,
        ),
        system_at(
            "2020-01-01T00:00:01.000Z",
            json!({ "addendum": "PROJECT ADDENDUM" }),
            Value::Null,
        ),
    ]
    .join("\n");

    let tree = parse_prompt_tree(&log, &dir.display().to_string()).unwrap();
    let (source, state) = tree.append_system.expect("addendum attributed");
    assert!(matches!(source, PromptSource::File(_)));
    assert_eq!(state, SourceState::Modified);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_source_older_than_the_prompt_is_fresh() {
    let dir = temp_dir("fresh");
    let pi_dir = dir.join(".pi");
    std::fs::create_dir_all(&pi_dir).unwrap();
    std::fs::write(pi_dir.join("APPEND_SYSTEM.md"), "PROJECT ADDENDUM\n").unwrap();

    let log = system_at(
        "2099-01-01T00:00:00.000Z",
        json!({ "preamble": "p", "tools": "t", "addendum": "PROJECT ADDENDUM" }),
        Value::Null,
    );

    let tree = parse_prompt_tree(&log, &dir.display().to_string()).unwrap();
    let (_, state) = tree.append_system.expect("addendum attributed");
    assert_eq!(state, SourceState::Ok);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn context_files_report_drift_too() {
    let dir = temp_dir("ctxdrift");
    let agents = dir.join("AGENTS.md");
    std::fs::write(&agents, "project rules\n").unwrap();
    let project_context = format!(
        "Project-specific instructions and guidelines:\n\n<project_instructions path=\"{}\">\nproject rules\n</project_instructions>",
        agents.display()
    );

    let log = system_at(
        "2020-01-01T00:00:00.000Z",
        json!({ "preamble": "p", "tools": "t", "project_context": project_context }),
        Value::Null,
    );

    let tree = parse_prompt_tree(&log, &dir.display().to_string()).unwrap();
    assert_eq!(tree.context_files.len(), 1);
    assert_eq!(tree.context_files[0].state, SourceState::Modified);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_addendum_preview_and_inner_size_survive_without_a_sidecar() {
    let log = system(
        json!({
            "preamble": "p",
            "tools": "t",
            "addendum": "<addendum>\nCIM BUDU RULES\n</addendum>",
        }),
        Value::Null,
    );

    let tree = parse_prompt_tree(&log, "D:/work").expect("tree");
    assert_eq!(tree.addendum_preview, "CIM BUDU RULES");
    let addendum = tree
        .sections
        .iter()
        .find(|section| section.name == "addendum")
        .expect("addendum");
    assert_eq!(
        addendum.chars,
        "CIM BUDU RULES".len(),
        "the wrapper is not part of the section body"
    );
}

#[test]
fn cli_prompt_files_under_agents_are_attributed_by_content() {
    let home = temp_dir("agents-home");
    let agents = home.join(".pi").join("agent").join("agents");
    std::fs::create_dir_all(&agents).unwrap();
    std::fs::write(agents.join("cim-budu.md"), "CIM BUDU RULES\n").unwrap();
    std::fs::write(agents.join("unrelated.md"), "something else\n").unwrap();

    let candidates = super::sources::candidate_paths("D:/work", "APPEND_SYSTEM.md", Some(&home));
    // The canonical locations are checked first, the agents/ files last.
    let first = candidates[0].to_string_lossy().replace('\\', "/");
    assert!(first.ends_with(".pi/APPEND_SYSTEM.md"), "{first}");

    match super::sources::match_source("CIM BUDU RULES", &candidates) {
        Some(PromptSource::File(path)) => assert!(path.ends_with("cim-budu.md"), "{path}"),
        other => panic!("expected a file match, got {other:?}"),
    }
    assert!(
        super::sources::match_source("NOT IN ANY FILE 7f3a", &candidates).is_none(),
        "a file only wins when its text is the body"
    );

    std::fs::remove_dir_all(&home).ok();
}
