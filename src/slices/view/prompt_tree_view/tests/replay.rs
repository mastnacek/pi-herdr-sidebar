//! Transcript replay provider: frame, sources, previews, drift.

use super::*;

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
    assert!(rendered[0].contains("čekám na první tah"));
    assert_eq!(rendered.len(), 2, "hint line plus a spacer");
}

/// The noise rule, sharpened: a *canonical* discovery file that never reached
/// the engine gets exactly one actionable line, while speculative candidates
/// (`agents/*.md` and friends) are never listed at all.
#[test]
fn only_a_canonical_unloaded_file_gets_a_row() {
    let dir = temp_dir("unloaded");
    let pi_dir = dir.join(".pi");
    std::fs::create_dir_all(&pi_dir).unwrap();
    // Canonical location, on disk, but the prompt never carried it.
    std::fs::write(pi_dir.join("APPEND_SYSTEM.md"), "NOT LOADED\n").unwrap();

    let log = system(r#"{"preamble":"p","tools":"t","cwd":"D:/work"}"#, "");
    let rows = flat(&tree_from(&log, &dir.display().to_string()));
    let hints: Vec<&String> = rows
        .iter()
        .filter(|row| row.contains("APPEND_SYSTEM.md"))
        .collect();
    assert_eq!(
        hints.len(),
        1,
        "one line, not a directory listing: {rows:#?}"
    );
    assert!(hints[0].contains("/reload"), "{:?}", hints[0]);

    // An append that matched no file stays inline: candidates are not listed.
    let log = system(
        r#"{"preamble":"p","tools":"t","addendum":"<addendum>\nCIM BUDU RULES\n</addendum>"}"#,
        "",
    );
    let rendered = joined(&tree_from(&log, "D:/work"));
    assert!(rendered.contains("← inline"), "{rendered}");
    assert!(!rendered.contains("agents/"), "{rendered}");
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

/// The replay provider carries previews and sizes too, so an inline append is
/// named even with no extension publishing the sidecar.
#[test]
fn replay_shows_the_inline_addendum_preview_and_size() {
    let log = system(
        r#"{"preamble":"p","tools":"t","addendum":"<addendum>\nCIM BUDU RULES\n</addendum>"}"#,
        "",
    );
    let rendered = joined(&tree_from(&log, "D:/work"));

    assert!(rendered.contains("replay ze session logu"), "{rendered}");
    assert!(rendered.contains("← inline"), "{rendered}");
    assert!(rendered.contains("CIM BUDU RULES"), "{rendered}");
    assert!(
        rendered.contains("(14 zn)"),
        "the body size, wrapper excluded: {rendered}"
    );
}

#[test]
fn replay_names_the_append_when_its_text_matches_a_file() {
    let dir = temp_dir("replayfile");
    let pi_dir = dir.join(".pi");
    std::fs::create_dir_all(&pi_dir).unwrap();
    std::fs::write(pi_dir.join("APPEND_SYSTEM.md"), "CIM BUDU RULES\n").unwrap();

    let log = system(
        r#"{"preamble":"p","tools":"t","addendum":"<addendum>\nCIM BUDU RULES\n</addendum>"}"#,
        "",
    );
    let rendered = joined(&tree_from(&log, &dir.display().to_string()));

    assert!(rendered.contains("APPEND_SYSTEM.md"), "{rendered}");
    assert!(rendered.contains("(14 zn)"), "{rendered}");

    std::fs::remove_dir_all(&dir).ok();
}

/// Both providers explain an addendum that exists on disk but never reached the
/// engine — the answer to "why is my append missing?".
#[test]
fn an_absent_addendum_points_at_the_file_that_never_loaded() {
    let dir = temp_dir("hintview");
    let pi_dir = dir.join(".pi");
    std::fs::create_dir_all(&pi_dir).unwrap();
    std::fs::write(pi_dir.join("APPEND_SYSTEM.md"), "NEVER SENT\n").unwrap();
    let cwd = dir.display().to_string();

    let log = system(r#"{"preamble":"p","tools":"t"}"#, "");
    let replay = joined(&tree_from(&log, &cwd));
    assert!(replay.contains("nepřítomno"), "{replay}");
    assert!(replay.contains("APPEND_SYSTEM.md"), "{replay}");
    assert!(replay.contains("/reload"), "{replay}");

    let raw = serde_json::json!({
        "version": 1,
        "live": true,
        "capturedAt": "2026-09-28T10:31:14.000Z",
        "cwd": cwd,
        "sections": ["preamble"],
        "sectionChars": { "preamble": 1 },
        "systemPromptChars": 1,
    })
    .to_string();
    let exact: PromptSidecar = serde_json::from_str(&raw).expect("sidecar");
    let text: String = prompt_tree_lines(&LiveTelemetry::default(), Some(&exact))
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("APPEND_SYSTEM.md"), "{text}");
    assert!(
        text.contains("na disku, ale engine addendum neodeslal"),
        "{text}"
    );

    std::fs::remove_dir_all(&dir).ok();
}
