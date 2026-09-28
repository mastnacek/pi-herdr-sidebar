//! Unit tests for the `.prompt.json` sidecar reader.

use super::*;

/// Template without the keys individual tests override (`version`, `live`,
/// `forced`, `customPrompt`), since serde rejects a duplicate field.
fn sidecar_json(extra: &str) -> String {
    format!(
        r#"{{
  "source": "before_agent_start",
  "capturedAt": "2026-09-28T10:31:14.000Z",
  "appendSystemPrompt": {{ "chars": 17, "source": "file", "path": "D:/work/.pi/APPEND_SYSTEM.md", "preview": "PROJECT ADDENDUM" }},
  "sections": ["preamble", "tools", "rules"],
  "sectionChars": {{ "preamble": 169, "tools": 8300, "rules": 4100 }},
  "contextFiles": [{{ "path": "D:/work/AGENTS.md", "chars": 470 }}],
  "tools": ["read", "bash"],
  "skills": [{{ "name": "herdr-plugin-dev", "descriptionChars": 120 }}],
  "systemPromptChars": 13000
  {extra}
}}"#
    )
}

fn live_v1() -> &'static str {
    r#", "version": 1, "live": true"#
}

fn parse(body: &str) -> PromptSidecar {
    serde_json::from_str(body).expect("sidecar json")
}

fn write_sidecar(dir: &std::path::Path, body: &str) -> PathBuf {
    let path = dir.join("pane.prompt.json");
    std::fs::write(&path, body).expect("write sidecar");
    path
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pi-sidebar-sidecar-{}-{}-{tag}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn parses_the_full_sidecar_shape() {
    let sidecar = parse(&sidecar_json(live_v1()));
    assert_eq!(sidecar.version, SIDECAR_VERSION);
    assert_eq!(sidecar.sections, vec!["preamble", "tools", "rules"]);
    assert_eq!(sidecar.section_chars("tools"), Some(8300));
    assert_eq!(sidecar.section_chars("addendum"), None);
    assert_eq!(sidecar.context_files.len(), 1);
    assert_eq!(sidecar.context_files[0].path, "D:/work/AGENTS.md");
    assert_eq!(sidecar.context_files[0].chars, 470);
    assert_eq!(sidecar.tools, vec!["read", "bash"]);
    assert_eq!(sidecar.skills[0].name, "herdr-plugin-dev");
    assert_eq!(sidecar.skills[0].description_chars, 120);
    assert_eq!(sidecar.system_prompt_chars, 13000);
    assert!(!sidecar.forced);
    assert!(sidecar.custom_prompt.is_none());
}

#[test]
fn addendum_is_carried_with_its_size_and_file() {
    let sidecar = parse(&sidecar_json(live_v1()));
    let text = sidecar.append_system_prompt.as_ref().expect("addendum");
    assert_eq!(text.chars, 17);
    assert_eq!(text.preview, "PROJECT ADDENDUM");
    assert!(text.is_file());

    match sidecar.attribute(&sidecar.append_system_prompt) {
        Some((PromptSource::File(path), _)) => assert!(path.ends_with("APPEND_SYSTEM.md")),
        other => panic!("expected a file source, got {other:?}"),
    }
}

#[test]
fn a_forced_prompt_is_flagged_and_custom_prompt_carries_the_text() {
    let sidecar = parse(&sidecar_json(
        r#", "version": 1, "live": true, "forced": true, "customPrompt": { "chars": 12, "source": "inline" }"#,
    ));
    assert!(sidecar.forced);
    assert_eq!(
        sidecar.attribute(&sidecar.custom_prompt),
        Some((PromptSource::Inline, SourceState::Ok))
    );
}

#[test]
fn an_inline_text_source_keeps_its_preview() {
    let sidecar = parse(&sidecar_json(
        r#", "version": 1, "live": true, "customPrompt": { "chars": 20, "source": "inline", "preview": "CIM BUDU" }"#,
    ));
    let custom = sidecar.custom_prompt.as_ref().expect("custom prompt");
    assert!(!custom.is_file());
    assert_eq!(custom.preview, "CIM BUDU");
    assert_eq!(custom.path, None);
}

#[test]
fn a_dead_or_too_new_sidecar_is_ignored() {
    let dir = temp_dir("dead");

    let path = write_sidecar(&dir, &sidecar_json(r#", "version": 1, "live": false"#));
    assert!(PromptSidecar::read_from_file(&path).is_none());

    let path = write_sidecar(&dir, &sidecar_json(r#", "version": 99, "live": true"#));
    assert!(PromptSidecar::read_from_file(&path).is_none());

    assert!(PromptSidecar::read_from_file(&dir.join("missing.json")).is_none());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_file_edited_after_the_capture_is_flagged_modified() {
    let dir = temp_dir("drift");
    let pi_dir = dir.join(".pi");
    std::fs::create_dir_all(&pi_dir).expect("pi dir");
    std::fs::write(pi_dir.join("APPEND_SYSTEM.md"), "PROJECT ADDENDUM\n").expect("write");

    // JSON needs the Windows separators escaped, and the capture time must
    // predate the file for the drift check to fire.
    let body = sidecar_json(live_v1())
        .replace(
            "D:/work/.pi/APPEND_SYSTEM.md",
            &pi_dir
                .join("APPEND_SYSTEM.md")
                .display()
                .to_string()
                .replace('\\', "\\\\"),
        )
        .replace("2026-09-28T10:31:14.000Z", "2020-01-01T00:00:00.000Z");
    let path = write_sidecar(&dir, &body);

    let sidecar = PromptSidecar::read_from_file(&path).expect("sidecar");
    match sidecar.attribute(&sidecar.append_system_prompt) {
        Some((_, SourceState::Modified)) => {}
        other => panic!("expected drift, got {other:?}"),
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn sidecar_path_derives_from_the_pane_snapshot_path() {
    assert_eq!(
        sidecar_path(Path::new("/state/w1M_p1.json")),
        PathBuf::from("/state/w1M_p1.prompt.json")
    );
    assert_eq!(
        sidecar_path(Path::new("C:/state/sidebar.json")),
        PathBuf::from("C:/state/sidebar.prompt.json")
    );
}
