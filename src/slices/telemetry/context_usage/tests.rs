//! Unit tests for the context accounting rules.

use super::*;

fn entries(jsonl: &str) -> Vec<Entry> {
    jsonl
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn assistant(id: &str, total: u64, extra: &str) -> String {
    format!(
        r#"{{"type":"message","id":"{id}","message":{{"role":"assistant","stopReason":"stop","model":"m","provider":"p","usage":{{"input":10,"output":20,"cacheRead":0,"cacheWrite":0,"totalTokens":{total}}}{extra}}}}}"#
    )
}

/// Assistant whose usage has no tokens at all (pi rejects it as context).
fn empty_assistant(id: &str) -> String {
    format!(
        r#"{{"type":"message","id":"{id}","message":{{"role":"assistant","stopReason":"stop","usage":{{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0}}}}}}"#
    )
}

#[test]
fn last_assistant_usage_wins() {
    let log = format!(
        "{}\n{}\n",
        assistant("a1", 1000, ""),
        assistant("a2", 2500, "")
    );
    assert_eq!(estimate_context_tokens(&entries(&log)), Some(2500));
}

#[test]
fn aborted_and_errored_usages_are_ignored() {
    let log = format!(
        "{}\n{}\n{}\n",
        assistant("a1", 1000, ""),
        assistant("a2", 9000, "").replace("\"stop\"", "\"aborted\""),
        assistant("a3", 8000, "").replace("\"stop\"", "\"error\"")
    );
    assert_eq!(estimate_context_tokens(&entries(&log)), Some(1000));
}

#[test]
fn all_zero_usage_assistant_is_ignored() {
    let log = format!("{}\n{}\n", assistant("a1", 1000, ""), empty_assistant("a2"));
    assert_eq!(estimate_context_tokens(&entries(&log)), Some(1000));
}

#[test]
fn trailing_messages_are_estimated() {
    // 400 chars of tool result after the usage -> 100 estimated tokens.
    let log = format!(
        "{}\n{}\n",
        assistant("a1", 1000, ""),
        format!(
            r#"{{"type":"message","id":"t1","message":{{"role":"toolResult","content":[{{"type":"text","text":"{}"}}]}}}}"#,
            "x".repeat(400)
        )
    );
    assert_eq!(estimate_context_tokens(&entries(&log)), Some(1100));
}

#[test]
fn context_edit_after_usage_keeps_last_measurement() {
    // pi would re-estimate the projected transcript here, which needs the
    // system prompt we cannot see; keeping the measurement is the honest
    // fallback (and never blanks the gauge mid-session).
    let log = format!(
        "{}\n{}\n",
        assistant("a1", 400_000, ""),
        r#"{"type":"context_edit","id":"e1","targetId":"t1","replacement":null}"#
    );
    assert_eq!(estimate_context_tokens(&entries(&log)), Some(400_000));
}

#[test]
fn usage_after_context_edit_is_trusted() {
    let log = format!(
        "{}\n{}\n{}\n",
        assistant("a1", 400_000, ""),
        r#"{"type":"context_edit","id":"e1","targetId":"t1","replacement":null}"#,
        assistant("a2", 5000, "")
    );
    assert_eq!(estimate_context_tokens(&entries(&log)), Some(5000));
}

#[test]
fn compaction_after_usage_makes_it_unknown() {
    let log = format!(
        "{}\n{}\n",
        assistant("a1", 400_000, ""),
        r#"{"type":"compaction","id":"c1","summary":"compressed"}"#
    );
    assert_eq!(estimate_context_tokens(&entries(&log)), None);
}

#[test]
fn omission_removes_trailing_target_tokens() {
    let big_tool_result = format!(
        r#"{{"type":"message","id":"t1","message":{{"role":"toolResult","content":[{{"type":"text","text":"{}"}}]}}}}"#,
        "x".repeat(400)
    );
    let log = format!(
        "{}\n{}\n{}\n",
        assistant("a1", 1000, ""),
        big_tool_result,
        r#"{"type":"context_edit","id":"e1","targetId":"t1","replacement":null}"#
    );
    // The omitted tool result stops contributing its 400 chars (100 tokens).
    assert_eq!(estimate_context_tokens(&entries(&log)), Some(1000));
}

#[test]
fn edits_before_the_usage_do_not_change_it() {
    let log = format!(
        "{}\n{}\n{}\n",
        assistant("a1", 900, ""),
        r#"{"type":"context_edit","id":"e1","targetId":"a1","replacement":{"content":[{"type":"text","text":"tiny"}]}}"#,
        assistant("a2", 2500, "")
    );
    assert_eq!(estimate_context_tokens(&entries(&log)), Some(2500));
}

#[test]
fn replacement_swaps_trailing_content() {
    let log = format!(
        "{}\n{}\n{}\n",
        assistant("a1", 1000, ""),
        r#"{"type":"message","id":"t1","message":{"role":"toolResult","content":[{"type":"text","text":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}]}}"#,
        r#"{"type":"context_edit","id":"e1","targetId":"t1","replacement":{"content":[{"type":"text","text":"short"}]}}"#
    );
    // 5 chars -> 2 tokens instead of the original 10.
    assert_eq!(estimate_context_tokens(&entries(&log)), Some(1002));
}

#[test]
fn no_usage_at_all_is_unknown() {
    let log = r#"{"type":"session","id":"s1","cwd":"C:\\tmp"}
{"type":"message","id":"u1","message":{"role":"user","content":"hi"}}
"#;
    assert_eq!(estimate_context_tokens(&entries(log)), None);
}

/// End-to-end pass over real session logs, with and without context-mode.
///
/// Ignored by default because it reads the developer's `~/.pi/agent/sessions`.
/// Run with `cargo test -- --ignored real_sessions`.
#[test]
#[ignore = "reads the developer's real ~/.pi/agent/sessions"]
fn real_sessions_parse_with_and_without_context_edits() {
    let Some(home) = crate::shared::dirs_home() else {
        return;
    };
    let Ok(projects) = std::fs::read_dir(home.join(".pi").join("agent").join("sessions")) else {
        return;
    };

    let (mut with_edits, mut known_with_edits) = (0usize, 0usize);
    let (mut without_edits, mut known_without_edits) = (0usize, 0usize);

    for project in projects.flatten() {
        let Ok(files) = std::fs::read_dir(project.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().is_none_or(|ext| ext != "jsonl") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some(live) = crate::slices::telemetry::parse_session(&path, "") else {
                continue;
            };
            if text.contains("\"context_edit\"") {
                with_edits += 1;
                known_with_edits += usize::from(!live.context_unknown);
            } else {
                without_edits += 1;
                known_without_edits += usize::from(!live.context_unknown);
            }
            if !live.context_unknown && live.context_window > 0 {
                assert!(
                    live.context_percent.is_some_and(|pct| pct <= 150.0),
                    "implausible context percent for {}",
                    path.display()
                );
            }
        }
    }

    assert!(without_edits > 0, "no plain sessions found to check");
    assert!(
        with_edits == 0 || known_with_edits > 0,
        "context-mode sessions must not all be unknown ({known_with_edits}/{with_edits} known)"
    );
    eprintln!(
        "context_edit sessions: {known_with_edits}/{with_edits} known; \
         plain sessions: {known_without_edits}/{without_edits} known"
    );
}
