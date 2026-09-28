//! Unit tests for the system-prompt replay.

use super::*;
use serde_json::{json, Value};

/// Serialize one JSON entry to a session line.
fn line(entry: Value) -> String {
    entry.to_string()
}

fn system(sections: Value, extra: Value) -> String {
    let mut message = json!({ "role": "system", "content": "", "sections": sections });
    if let Some(obj) = extra.as_object() {
        for (k, v) in obj {
            message[k] = v.clone();
        }
    }
    line(json!({ "type": "message", "message": message }))
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
    assert_eq!(tree.append_system, Some(PromptSource::Inline));
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
        Some(PromptSource::File(path)) => assert!(path.ends_with("APPEND_SYSTEM.md")),
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
    assert_eq!(tree.system_override, Some(PromptSource::Inline));
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
