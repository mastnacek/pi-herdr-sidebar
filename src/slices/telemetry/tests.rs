//! Session-parsing tests for the counters the banner shows.

use super::*;

/// Minimal session: one user turn, two tool invocations, one failing result.
fn session_log(cwd: &str) -> String {
    let lines = [
        format!(
            r#"{{"type":"session","version":3,"id":"s1","timestamp":"2026-09-28T10:00:00.000Z","cwd":"{cwd}"}}"#
        ),
        r#"{"type":"message","timestamp":"2026-09-28T10:00:01.000Z","message":{"role":"user","content":"hi","timestamp":1}}"#.to_string(),
        r#"{"type":"message","timestamp":"2026-09-28T10:00:02.000Z","message":{"role":"assistant","provider":"p","model":"m","stopReason":"stop","content":[{"type":"toolCall","name":"read","arguments":{}},{"type":"toolCall","name":"bash","arguments":{}}],"usage":{"input":10,"output":5,"cacheRead":0,"cacheWrite":0,"totalTokens":15,"cost":{"total":0.001}},"timestamp":2}}"#.to_string(),
        r#"{"type":"message","timestamp":"2026-09-28T10:00:03.000Z","message":{"role":"toolResult","toolCallId":"c1","toolName":"read","isError":false,"content":[{"type":"text","text":"ok"}],"timestamp":3}}"#.to_string(),
        r#"{"type":"message","timestamp":"2026-09-28T10:00:04.000Z","message":{"role":"toolResult","toolCallId":"c2","toolName":"bash","isError":true,"content":[{"type":"text","text":"boom"}],"timestamp":4}}"#.to_string(),
    ];
    lines.join("\n") + "\n"
}

fn parse(log: &str) -> LiveTelemetry {
    let path = std::env::temp_dir().join(format!(
        "pi-sidebar-session-{}-{}.jsonl",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::write(&path, log).expect("write session");
    let parsed = parse_session(&path, "s1").expect("session");
    std::fs::remove_file(&path).ok();
    parsed
}

/// pi writes `toolCall` / `toolResult`; the snake_case spellings are accepted
/// for compatibility, but only the camelCase ones ever appear in a real log.
#[test]
fn tool_invocations_and_errors_are_counted() {
    let cwd = std::env::temp_dir().display().to_string();
    let t = parse(&session_log(&cwd));

    assert_eq!(t.turns_count, 1);
    assert_eq!(
        t.tool_calls_count, 2,
        "one per invocation, not one per call plus one per result"
    );
    assert_eq!(t.tool_errors_count, 1, "only the isError result");
    assert_eq!(t.output_tokens, 5);
    assert_eq!(t.total_cost, 0.001);
}

#[test]
fn snake_case_spellings_still_count() {
    let cwd = std::env::temp_dir().display().to_string();
    let log = session_log(&cwd)
        .replace("\"toolCall\"", "\"tool_call\"")
        .replace("\"toolResult\"", "\"tool_result\"");
    let t = parse(&log);

    assert_eq!(t.tool_calls_count, 2);
    assert_eq!(t.tool_errors_count, 1);
}
