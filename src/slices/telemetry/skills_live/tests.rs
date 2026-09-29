//! Unit + dev-preview tests for the live Skills telemetry parser.

use std::path::Path;

use super::gates::strip_comments;
use super::time::iso_to_epoch_ms;
use super::{parse_session_skills, parse_skills_str};

const SAMPLE: &str = r#"
{"type":"session","cwd":"D:\\proj","timestamp":"2026-09-23T21:00:00.000Z"}
{"type":"message","timestamp":"2026-09-23T21:00:01.000Z","message":{"role":"user","content":[{"type":"text","text":"go"}]}}
{"type":"message","timestamp":"2026-09-23T21:00:02.000Z","message":{"role":"assistant","content":[{"type":"toolCall","name":"read","arguments":{"path":"C:\\Users\\x\\.pi\\agent\\skills\\pi-plugin-dev\\SKILL.md"}}]}}
{"type":"message","timestamp":"2026-09-23T21:00:03.000Z","message":{"role":"assistant","content":[{"type":"toolCall","name":"read","arguments":{"path":"C:\\skill\\references\\tools-and-schema.md"}}]}}
{"type":"message","timestamp":"2026-09-23T21:00:04.000Z","message":{"role":"toolResult","content":[]}}
{"type":"message","timestamp":"2026-09-23T21:00:05.000Z","message":{"role":"assistant","content":[{"type":"toolCall","name":"edit","arguments":{"path":"src/tool.ts","edits":[{"newText":"pi.registerTool('x', { execute() { const s = Type.Object({ mode: Type.Union([Type.Literal('a'), Type.Literal('b')]) }); throw new Error('bad'); } })"}]}}]}}
{"type":"message","timestamp":"2026-09-23T21:00:06.000Z","message":{"role":"assistant","content":[{"type":"toolCall","name":"bash","arguments":{"command":"cargo test"}}]}}
{"type":"message","timestamp":"2026-09-23T21:00:07.000Z","message":{"role":"assistant","content":[{"type":"text","text":"done"}]}}
"#;

#[test]
fn derives_skill_state_from_session() {
    let f = super::parse_skills_str(SAMPLE);
    assert!(f.live);
    let s = f.state.as_ref().unwrap();

    assert_eq!(s.active_skill.as_deref(), Some("pi-plugin-dev"));
    // SKILL.md read activates the skill (action, not a reference); the
    // reference/ read adds one deduped reference.
    assert_eq!(s.references.len(), 1);
    assert!(s
        .references
        .iter()
        .any(|r| r.name == "tools-and-schema.md" && r.summary.contains("StringEnum")));
    // read(SKILL.md) + read(reference) + edit + (bash truncated out of 8-window)
    assert!(!s.actions.is_empty());
    assert_eq!(s.turn_count, 1);
    assert!(!s.in_turn, "final assistant text message = settled");
    // SKILL.md + reference reads short-circuit before the inspect counter.
    assert_eq!(s.inspected_count, 0);
    assert_eq!(s.modified_count, 1);

    // Gates: Type.Union of Type.Literal must fail the StringEnum rule.
    assert!(s
        .compliance
        .iter()
        .any(|c| c.rule == "string-enum" && c.status == "fail"));
    assert!(s
        .compliance
        .iter()
        .any(|c| c.rule == "error-throw" && c.status == "pass"));

    // Elapsed window: first toolCall 21:00:02 → last entry 21:00:07 = 5000ms.
    assert_eq!(s.last_update_time - s.start_time, 5000);
    let (passed, total) = f.gate_score();
    assert_eq!(total, s.compliance.len());
    assert!(passed < total); // one failing gate
}

/// Dev test: `PI_SIDEBAR_SESSION_FILE=<session.jsonl> cargo test \
/// parses_real_session_skills -- --ignored --nocapture`
/// Skips quietly when the env var is absent so a plain `--ignored` run
/// stays green.
#[test]
#[ignore]
fn parses_real_session_skills() {
    let Ok(path) = std::env::var("PI_SIDEBAR_SESSION_FILE") else {
        println!("skipped: set PI_SIDEBAR_SESSION_FILE=<session.jsonl> to inspect a real session");
        return;
    };
    let f = super::parse_session_skills(Path::new(&path)).expect("parse");
    let s = f.state.as_ref().unwrap();
    println!("active: {:?}", s.active_skill);
    println!("refs: {}", s.references.len());
    println!(
        "actions: {} (last: {:?})",
        s.actions.len(),
        s.actions.last().map(|a| (&a.kind, &a.target))
    );
    println!("gates: {:?}", f.gate_score());
    println!(
        "inspected: {} modified: {} turns: {} inTurn: {}",
        s.inspected_count, s.modified_count, s.turn_count, s.in_turn
    );
    assert!(s.turn_count > 0);
}

#[test]
fn iso_parse() {
    assert_eq!(
        iso_to_epoch_ms("2026-09-23T21:02:02.995Z"),
        Some(1_790_197_322_995)
    );
    assert_eq!(iso_to_epoch_ms("not-a-date"), None);
}

#[test]
fn strip_comments_keeps_urls() {
    let code = "let u = \"https://x.y/a\"; // real comment\n/* block */ let b = 1;";
    let out = strip_comments(code);
    assert!(out.contains("https://x.y/a"));
    assert!(!out.contains("real comment"));
    assert!(!out.contains("block"));
}
