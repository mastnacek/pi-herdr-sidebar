//! Per-line parsing of pi session JSONL records.
//!
//! Two rules keep this cheap enough to run over ~300 MB of logs:
//!
//! * substring gates decide whether a line is worth handing to the JSON parser
//!   (most bytes are assistant text and tool results we never need), and
//! * skill paths are read out of the **raw** line, which avoids deserialising
//!   `arguments` — by far the largest field in a tool-call record.
use super::model::plugin_of;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct SessionLine {
    #[serde(rename = "type")]
    kind: Option<String>,
    message: Option<MsgLine>,
}
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct MsgLine {
    role: Option<String>,
    content: Option<Content>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Content {
    Parts(Vec<Part>),
    Text(String),
}

#[derive(Debug, Deserialize)]
struct Part {
    #[serde(rename = "type")]
    kind: Option<String>,
    name: Option<String>,
    text: Option<String>,
}

/// Extracts the skill name from the first `.../skills/<name>/SKILL.md` path
/// appearing in a raw line.
fn skill_from_raw_line(line: &str) -> Option<String> {
    const MARKER: &str = "/SKILL.md";
    const SKILL_DIR: &str = "/skills/";

    let mut search = 0usize;
    while let Some(rel) = line[search..].find(MARKER) {
        let at = search + rel;
        if let Some(sk) = line[..at].rfind(SKILL_DIR) {
            let name = line[sk + SKILL_DIR.len()..at].split('/').next()?.trim();
            if !name.is_empty() && !name.contains(['<', '>', '"', '\\']) {
                return Some(name.to_string());
            }
        }
        search = at + MARKER.len();
    }
    None
}

/// Recognises a slash command typed by the user (`/goal`, `/skill:foo`, ...).
pub(crate) fn command_from_text(text: &str) -> Option<String> {
    let token = text.trim().lines().next()?.split_whitespace().next()?;
    if !token.starts_with('/') || token.len() < 2 || token.len() > 40 {
        return None;
    }
    let body = &token[1..];
    let ok = body
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':'));
    ok.then(|| token.to_string())
}

fn skill_from_command(command: &str) -> Option<String> {
    command
        .strip_prefix("/skill:")
        .map(|s| s.split([':', ' ']).next().unwrap_or("").to_string())
        .filter(|s| !s.is_empty())
}

/// Running totals for one scan pass.
#[derive(Debug, Default)]
pub struct Counters {
    pub tools: HashMap<String, u64>,
    pub skills: HashMap<String, u64>,
    pub commands: HashMap<String, u64>,
    pub messages: u64,
}

impl Counters {
    /// Records one tool name.
    pub fn add_tool(&mut self, name: &str) {
        *self.tools.entry(name.to_string()).or_insert(0) += 1;
    }

    /// Calls served by the agent's own tools rather than a plugin.
    pub fn builtin_calls(&self) -> u64 {
        self.tools
            .iter()
            .filter(|(name, _)| plugin_of(name) == "builtin")
            .map(|(_, count)| *count)
            .sum()
    }
}

/// Reads one session file into `counters`.
pub fn scan_file(path: &Path, counters: &mut Counters) {
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    let reader = std::io::BufReader::with_capacity(1 << 16, file);
    use std::io::BufRead;

    for line in reader.lines() {
        let Ok(line) = line else { continue };

        // Cheap substring gates keep the JSON parser off most of the bytes.
        if line.contains("\"type\": \"message\"") || line.contains("\"type\":\"message\"") {
            counters.messages += 1;
        }
        let has_tool = line.contains("\"toolCall\"");
        let is_user = line.len() < 4096 && line.contains("\"user\"");
        if !has_tool && !is_user {
            continue;
        }

        let Ok(parsed) = serde_json::from_str::<SessionLine>(&line) else {
            continue;
        };

        if has_tool {
            if let Some(parts) = parsed
                .message
                .as_ref()
                .and_then(|m| m.content.as_ref())
                .and_then(parts_of)
            {
                for part in parts {
                    if part.kind.as_deref() == Some("toolCall") {
                        if let Some(name) = part.name.as_deref() {
                            counters.add_tool(name);
                        }
                    }
                }
            }
            if let Some(skill) = skill_from_raw_line(&line) {
                *counters.skills.entry(skill).or_insert(0) += 1;
            }
        }

        if is_user {
            let is_user_role = parsed
                .message
                .as_ref()
                .and_then(|m| m.role.as_deref())
                .is_some_and(|r| r == "user");
            if is_user_role {
                if let Some(text) = first_text(&parsed) {
                    if let Some(command) = command_from_text(text) {
                        if let Some(skill) = skill_from_command(&command) {
                            *counters.skills.entry(skill).or_insert(0) += 1;
                        }
                        *counters.commands.entry(command).or_insert(0) += 1;
                    }
                }
            }
        }
    }
}

fn parts_of(content: &Content) -> Option<&[Part]> {
    match content {
        Content::Parts(parts) => Some(parts),
        Content::Text(_) => None,
    }
}

fn first_text(parsed: &SessionLine) -> Option<&str> {
    match parsed.message.as_ref()?.content.as_ref()? {
        Content::Text(text) => Some(text.as_str()),
        Content::Parts(parts) => parts
            .iter()
            .find(|p| p.kind.as_deref() == Some("text"))
            .and_then(|p| p.text.as_deref()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_skill_from_raw_tool_arguments() {
        let line = r#"{"arguments":{"path":"C:/x/.pi/agent/skills/pi-plugin-dev/SKILL.md"}}"#;
        assert_eq!(skill_from_raw_line(line).as_deref(), Some("pi-plugin-dev"));
        assert_eq!(skill_from_raw_line(r#"{"nope":true}"#), None);
        // template placeholders must not be reported as skills
        assert_eq!(skill_from_raw_line(r#"/skills/<name>/SKILL.md"#), None);
    }

    #[test]
    fn picks_the_first_skill_in_the_line() {
        let line = r#"["/skills/aaa/SKILL.md","/skills/bbb/SKILL.md"]"#;
        assert_eq!(skill_from_raw_line(line).as_deref(), Some("aaa"));
    }

    #[test]
    fn recognises_slash_commands_only() {
        assert_eq!(command_from_text("/goal do it").as_deref(), Some("/goal"));
        assert_eq!(
            command_from_text("/skill:foo").as_deref(),
            Some("/skill:foo")
        );
        assert_eq!(command_from_text("  /exit  ").as_deref(), Some("/exit"));
        assert_eq!(command_from_text("not a command"), None);
        assert_eq!(command_from_text("/"), None);
        assert_eq!(command_from_text("a/b"), None);
    }

    #[test]
    fn skill_from_slash_command() {
        assert_eq!(skill_from_command("/skill:foo").as_deref(), Some("foo"));
        assert_eq!(skill_from_command("/goal"), None);
    }

    #[test]
    fn counts_tools_skills_and_commands_from_a_real_shaped_file() {
        let dir = std::env::temp_dir().join(format!("usage_scan_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s.jsonl");
        let lines = [
            r#"{"type": "message", "message": {"role": "assistant", "content": [{"type": "toolCall", "name": "ctx_execute", "arguments": {"path": "/skills/pi-plugin-dev/SKILL.md"}}]}}"#,
            r#"{"type": "message", "message": {"role": "user", "content": [{"type": "text", "text": "/goal ship it"}]}}"#,
            r#"{"type": "message", "message": {"role": "assistant", "content": [{"type": "toolCall", "name": "bash", "arguments": {}}]}}"#,
        ];
        std::fs::write(&file, lines.join("\n")).unwrap();

        let mut counters = Counters::default();
        scan_file(&file, &mut counters);

        assert_eq!(counters.messages, 3);
        assert_eq!(counters.tools.get("ctx_execute"), Some(&1));
        assert_eq!(counters.tools.get("bash"), Some(&1));
        assert_eq!(counters.skills.get("pi-plugin-dev"), Some(&1));
        assert_eq!(counters.commands.get("/goal"), Some(&1));
        assert_eq!(counters.builtin_calls(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }
}
