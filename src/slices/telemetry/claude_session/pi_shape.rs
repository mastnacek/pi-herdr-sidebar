//! Claude Code transcript → pi session lines.
//!
//! The Skills and MCP faces replay pi's message trail (`toolCall` blocks,
//! `toolResult` messages, lower-case tool names with pi's argument names).
//! Rewriting a Claude transcript into that shape lets both parsers run on it
//! unchanged:
//!
//! | Claude Code                        | pi                                     |
//! |------------------------------------|----------------------------------------|
//! | `Read {file_path}`                 | `read {path}`                          |
//! | `Edit {file_path, new_string}`     | `edit {path, edits:[{newText}]}`       |
//! | `Write {file_path, content}`       | `write {path, content}`                |
//! | `Bash {command}`                   | `bash {command}`                       |
//! | `Skill {skill: "plugin:name"}`     | `read {path: "skills/name/SKILL.md"}`  |
//! | `tool_result {tool_use_id, is_error}` | `toolResult {toolCallId, isError}`  |
//!
//! Subagent (`isSidechain`) and meta lines are dropped: the faces follow the
//! main conversation.

use serde_json::{json, Value};

/// Rewrites a whole transcript; lines that carry no message are skipped.
pub fn to_pi_lines(content: &str) -> String {
    let mut out = String::new();
    for line in content.lines() {
        let Ok(e) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if e.get("isSidechain").and_then(Value::as_bool) == Some(true)
            || e.get("isMeta").and_then(Value::as_bool) == Some(true)
        {
            continue;
        }
        let ts = e.get("timestamp").cloned().unwrap_or(Value::Null);
        let content = e.get("message").and_then(|m| m.get("content"));
        let messages = match e.get("type").and_then(Value::as_str) {
            Some("assistant") => vec![assistant(content)],
            Some("user") => user(content),
            _ => continue,
        };
        for message in messages {
            let entry = json!({ "type": "message", "timestamp": ts, "message": message });
            out.push_str(&entry.to_string());
            out.push('\n');
        }
    }
    out
}

fn assistant(content: Option<&Value>) -> Value {
    let blocks: Vec<Value> = blocks(content)
        .iter()
        .filter_map(|b| match b.get("type").and_then(Value::as_str) {
            Some("tool_use") => Some(tool_call(b)),
            Some("text") => Some(json!({ "type": "text", "text": b.get("text") })),
            _ => None,
        })
        .collect();
    json!({ "role": "assistant", "content": blocks })
}

/// A prompt becomes one `user` message; every tool result its own
/// `toolResult` message, as pi writes them.
fn user(content: Option<&Value>) -> Vec<Value> {
    if let Some(Value::String(text)) = content {
        return vec![json!({ "role": "user", "content": [{ "type": "text", "text": text }] })];
    }
    let mut out = Vec::new();
    let mut prompt = false;
    for b in blocks(content) {
        match b.get("type").and_then(Value::as_str) {
            Some("tool_result") => out.push(json!({
                "role": "toolResult",
                "toolCallId": b.get("tool_use_id"),
                "isError": b.get("is_error").and_then(Value::as_bool).unwrap_or(false),
                "content": [{ "type": "text", "text": result_text(b.get("content")) }],
            })),
            Some("text") => prompt = true,
            _ => {}
        }
    }
    if prompt {
        out.insert(0, json!({ "role": "user", "content": [] }));
    }
    out
}

fn tool_call(b: &Value) -> Value {
    let name = b.get("name").and_then(Value::as_str).unwrap_or("");
    let input = b.get("input").cloned().unwrap_or(Value::Null);
    let path = input.get("file_path").cloned().unwrap_or(Value::Null);
    let (name, arguments) = match name {
        "Read" => ("read".to_string(), json!({ "path": path })),
        "Write" => ("write".to_string(), json!({ "path": path, "content": input.get("content") })),
        "Edit" => (
            "edit".to_string(),
            json!({ "path": path, "edits": [{ "newText": input.get("new_string") }] }),
        ),
        "Bash" => ("bash".to_string(), json!({ "command": input.get("command") })),
        "Skill" => {
            let skill = input.get("skill").and_then(Value::as_str).unwrap_or("skill");
            let skill = skill.rsplit(':').next().unwrap_or(skill);
            ("read".to_string(), json!({ "path": format!("skills/{skill}/SKILL.md") }))
        }
        other => (other.to_string(), input),
    };
    json!({ "type": "toolCall", "id": b.get("id"), "name": name, "arguments": arguments })
}

fn blocks(content: Option<&Value>) -> &[Value] {
    content.and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// Tool result text: a plain string or the text of its blocks.
fn result_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|i| i.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}
