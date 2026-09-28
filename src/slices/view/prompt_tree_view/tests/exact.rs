//! Exact provider: the engine's own numbers, straight from the sidecar.

use super::*;

/// The sidecar is the exact provider: it must win even with no transcript at
/// all, show real paths, and label itself as `exact`.
#[test]
fn the_sidecar_provider_renders_exact_rows() {
    let file = sidecar(
        r#"{
          "version": 1, "live": true,
          "capturedAt": "2026-09-28T10:31:14.000Z",
          "forced": false,
          "appendSystemPrompt": { "chars": 312, "source": "file", "path": "D:/work/.pi/APPEND_SYSTEM.md" },
          "sections": ["preamble", "tools", "addendum", "project_context", "skills", "cwd"],
          "sectionChars": { "preamble": 169, "tools": 8300, "addendum": 312, "project_context": 669, "skills": 4100, "cwd": 62 },
          "contextFiles": [ { "path": "D:/work/AGENTS.md", "chars": 470 } ],
          "tools": ["read", "bash", "edit"],
          "skills": [ { "name": "herdr-plugin-dev", "descriptionChars": 120 } ],
          "systemPromptChars": 13000
        }"#,
    );
    let text = prompt_tree_lines(&LiveTelemetry::default(), Some(&file))
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("exact · before_agent_start"), "{text}");
    assert!(text.contains("D:/work/AGENTS.md"), "{text}");
    assert!(text.contains("8.3k zn"), "{text}");
    assert!(text.contains("312 zn"), "{text}");
    assert!(text.contains("herdr-plugin-dev"), "{text}");
    assert!(text.contains("načteno 10:31:14"), "{text}");
    assert!(!text.contains("vynucený"), "{text}");
}

#[test]
fn an_inline_addendum_shows_its_preview() {
    let file = sidecar(
        r#"{
          "version": 1, "live": true,
          "capturedAt": "2026-09-28T10:31:14.000Z",
          "appendSystemPrompt": { "chars": 3879, "source": "inline", "preview": "CIM BUDU" },
          "sections": ["preamble", "addendum", "cwd"],
          "sectionChars": { "preamble": 169, "addendum": 3879, "cwd": 62 },
          "systemPromptChars": 4100
        }"#,
    );
    let text = prompt_tree_lines(&LiveTelemetry::default(), Some(&file))
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("← inline"), "{text}");
    assert!(
        text.contains("CIM BUDU"),
        "the preview names the append: {text}"
    );
    assert!(text.contains("3.9k zn"), "{text}");
}

#[test]
fn a_forced_prompt_is_flagged_in_the_header() {
    let file = sidecar(
        r#"{
          "version": 1, "live": true,
          "capturedAt": "2026-09-28T10:31:14.000Z",
          "forced": true,
          "customPrompt": { "chars": 12, "source": "inline" },
          "sections": ["preamble"],
          "sectionChars": { "preamble": 12 },
          "systemPromptChars": 12
        }"#,
    );
    let text = prompt_tree_lines(&LiveTelemetry::default(), Some(&file))
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("⚠ vynucený prompt"), "{text}");
    assert!(text.contains("tools"), "frame stays complete: {text}");
    assert!(text.contains("nepřítomno"), "{text}");
}
