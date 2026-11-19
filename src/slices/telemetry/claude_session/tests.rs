use super::*;

const LOG: &str = r#"{"type":"user","cwd":"D:\\work\\demo","sessionId":"abc12345-0000","timestamp":"2026-11-19T10:00:00Z","message":{"role":"user","content":"ahoj"}}
{"type":"assistant","cwd":"D:\\work\\demo","effort":"high","timestamp":"2026-11-19T10:00:05Z","message":{"id":"msg_1","model":"claude-opus-5-5","stop_reason":null,"usage":{"input_tokens":10,"output_tokens":20,"cache_read_input_tokens":1000,"cache_creation_input_tokens":500,"output_tokens_details":{"thinking_tokens":7}},"content":[{"type":"thinking","thinking":""}]}}
{"type":"assistant","cwd":"D:\\work\\demo","effort":"high","timestamp":"2026-11-19T10:00:06Z","message":{"id":"msg_1","model":"claude-opus-5-5","stop_reason":"tool_use","usage":{"input_tokens":10,"output_tokens":20,"cache_read_input_tokens":1000,"cache_creation_input_tokens":500},"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{}}]}}
{"type":"user","cwd":"D:\\work\\demo","timestamp":"2026-11-19T10:00:07Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":true,"content":"x"}]}}
{"type":"assistant","isSidechain":true,"timestamp":"2026-11-19T10:00:08Z","message":{"id":"msg_s","model":"claude-haiku-5-5","usage":{"input_tokens":3,"output_tokens":4,"cache_read_input_tokens":99999,"cache_creation_input_tokens":0},"content":[{"type":"tool_use","id":"t2","name":"Read","input":{}}]}}
{"type":"assistant","cwd":"D:\\work\\demo","effort":"high","timestamp":"2026-11-19T10:00:09Z","message":{"id":"msg_2","model":"claude-opus-5-5","stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":30,"cache_read_input_tokens":1600,"cache_creation_input_tokens":100},"content":[{"type":"text","text":"hotovo"}]}}
"#;

fn parsed() -> LiveTelemetry {
    parse_content(LOG, "").expect("telemetry")
}

#[test]
fn usage_counts_once_per_message_id() {
    let t = parsed();
    // msg_1 once + subagent msg_s + msg_2.
    assert_eq!(t.input_tokens, 10 + 3 + 1);
    assert_eq!(t.output_tokens, 20 + 4 + 30);
    assert_eq!(t.cache_read, 1000 + 99999 + 1600);
    assert_eq!(t.cache_write, 500 + 100);
    assert_eq!(t.reasoning_tokens, 7);
}

#[test]
fn context_is_last_main_response_not_subagent() {
    let t = parsed();
    assert_eq!(t.context_tokens, 1 + 1600 + 100);
    assert!(!t.context_unknown);
}

#[test]
fn model_effort_session_and_cwd() {
    let t = parsed();
    assert_eq!(t.provider, "anthropic");
    assert_eq!(t.model_id, "claude-opus-5-5");
    assert_eq!(t.thinking_level, "high");
    assert_eq!(t.session_id, "abc12345-0000");
    assert_eq!(t.cwd, "D:\\work\\demo");
    assert_eq!(t.last_entry_ts, "2026-11-19T10:00:09Z");
}

#[test]
fn turns_tools_and_errors() {
    let t = parsed();
    assert_eq!(t.turns_count, 1, "tool results are not turns");
    assert_eq!(t.tool_calls_count, 2, "main and subagent tool_use blocks");
    assert_eq!(t.tool_errors_count, 1);
}

#[test]
fn end_turn_means_idle_and_a_new_prompt_means_working() {
    assert!(!parsed().is_working);
    let more = format!(
        "{LOG}{}\n",
        r#"{"type":"user","message":{"role":"user","content":"dál"}}"#
    );
    assert!(parse_content(&more, "").unwrap().is_working);
}

#[test]
fn compact_boundary_makes_context_unknown() {
    let log = format!(
        "{LOG}{}\n",
        r#"{"type":"system","subtype":"compact_boundary","content":"Conversation compacted"}"#
    );
    let t = parse_content(&log, "").unwrap();
    assert!(t.context_unknown);
    assert_eq!(t.context_percent, None);
}

#[test]
fn no_response_yields_none() {
    let log = r#"{"type":"user","message":{"role":"user","content":"ahoj"}}"#;
    assert!(parse_content(log, "").is_none());
}

#[test]
fn slug_replaces_every_non_alphanumeric() {
    assert_eq!(project_slug(r"D:\01_programovani\dejaview"), "D--01-programovani-dejaview");
}

#[test]
fn finds_file_in_cwd_folder_then_anywhere() {
    let root = std::env::temp_dir().join(format!("claude-session-test-{}", std::process::id()));
    let sid = "4bd4e17c-285d-4a72-950b-98eb81004494";
    let other = root.join("D--elsewhere");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join(format!("{sid}.jsonl")), "").unwrap();

    let found = find_session_file_in(&root, sid, Some(r"D:\work")).unwrap();
    assert_eq!(found, other.join(format!("{sid}.jsonl")));
    assert!(find_session_file_in(&root, "short", None).is_none());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn transcript_path_is_recognised() {
    assert!(is_transcript(Path::new("C:/Users/x/.claude/projects/D--a/abc.jsonl")));
    assert!(!is_transcript(Path::new("C:/Users/x/.pi/agent/sessions/--D--a--/abc.jsonl")));
}

const TOOLS: &str = r#"{"type":"user","timestamp":"2026-11-19T10:00:00Z","message":{"role":"user","content":"udělej to"}}
{"type":"assistant","timestamp":"2026-11-19T10:00:01Z","message":{"id":"m1","content":[{"type":"tool_use","id":"s1","name":"Skill","input":{"skill":"cc-herdr-plugin-dev:herdr-plugin-dev"}}]}}
{"type":"assistant","timestamp":"2026-11-19T10:00:02Z","message":{"id":"m2","content":[{"type":"tool_use","id":"c1","name":"mcp__plugin_context7_context7__query-docs","input":{"query":"ratatui list"}}]}}
{"type":"user","timestamp":"2026-11-19T10:00:03Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"c1","is_error":true,"content":[{"type":"text","text":"12345678"}]}]}}
{"type":"assistant","timestamp":"2026-11-19T10:00:04Z","message":{"id":"m3","content":[{"type":"tool_use","id":"e1","name":"Edit","input":{"file_path":"D:\\x\\src\\a.rs","old_string":"a","new_string":"b"}}]}}
{"type":"assistant","isSidechain":true,"timestamp":"2026-11-19T10:00:05Z","message":{"id":"m4","content":[{"type":"tool_use","id":"k1","name":"mcp__knowledge-base__kb_search","input":{"query":"x"}}]}}
"#;

#[test]
fn skill_tool_activates_the_skill_and_edit_counts_as_modification() {
    let snap = crate::slices::telemetry::skills_live::parse_skills_str(&pi_shape::to_pi_lines(TOOLS));
    let state = snap.state.unwrap();
    assert_eq!(state.active_skill.as_deref(), Some("herdr-plugin-dev"));
    assert_eq!(state.modified_count, 1);
    assert_eq!(state.turn_count, 1, "tool results are not turns");
}

#[test]
fn mcp_calls_split_server_and_tool_and_skip_subagents() {
    let t = crate::slices::telemetry::mcp_live::parse_mcp_str(&pi_shape::to_pi_lines(TOOLS)).unwrap();
    assert_eq!(t.total_calls, 1);
    assert_eq!(t.total_errors, 1);
    assert_eq!(t.servers_used, vec!["context7".to_string()]);
    let call = &t.recent_calls[0];
    assert_eq!((call.badge.as_str(), call.tool.as_str()), ("C7", "query-docs"));
    assert_eq!(call.summary, "\"ratatui list\"");
    assert_eq!(call.payload_tokens, 2);
    assert!(!t.in_flight);
}

#[test]
fn mcp_names_from_claude_code() {
    use crate::slices::telemetry::mcp_live::classify_mcp_tool;
    let c = |n: &str| classify_mcp_tool(n, &[]).unwrap();
    assert_eq!(c("mcp__rea__open_binary"), ("rea".into(), "open_binary".into()));
    assert_eq!(c("mcp__plugin_cc-uma-memory_uma__uma_write"), ("uma".into(), "uma_write".into()));
    assert_eq!(c("mcp__knowledge-base__kb_search"), ("knowledge-base".into(), "kb_search".into()));
}

#[test]
fn subscription_label_from_claude_json() {
    use super::subscription::label_from;
    let max = r#"{"oauthAccount":{"organizationType":"claude_max","organizationRateLimitTier":"default_claude_max_5x","billingType":"stripe_subscription"}}"#;
    assert_eq!(label_from(max), "Claude Code · Max 5x");
    let pro = r#"{"oauthAccount":{"organizationType":"claude_pro","organizationRateLimitTier":"default_claude_pro"}}"#;
    assert_eq!(label_from(pro), "Claude Code · Pro");
    assert_eq!(label_from(r#"{"numStartups":3}"#), "Claude Code · API");
    assert_eq!(label_from(""), "Claude Code · API");
}
