//! Independent Skills telemetry: derive the Skills face state directly from the
//! Pi session JSONL — the same source of truth the Status face uses. No TS
//! extension, sidecar, or event bus required.
//!
//! Replays pi-plugin-dev's tracker heuristics over the session tool-call trail:
//! - `read` on a `SKILL.md`          → skill activation
//! - `read` under `/references/`     → guidance reference loaded
//! - `read` in pi engine docs        → `doc_consult` action
//! - `read` of any other file        → inspected file
//! - `edit` / `write`                → modification + compliance gates
//! - `bash`                          → shell verification action

use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::skills::{ComplianceCheck, SkillAction, SkillReference, SkillSnapshotFile, SkillState};

/// Actions kept for the Focus list (mirrors the TS publisher's `slice(-8)`).
const MAX_ACTIONS: usize = 8;

/// Parse a session JSONL file into Skills-face state. Never fails hard: an
/// empty/unknown file yields an empty live state rather than `None`.
pub fn parse_session_skills(path: &Path) -> Option<SkillSnapshotFile> {
    let content = fs::read_to_string(path).ok()?;
    Some(parse_skills_str(&content))
}

/// Parse session JSONL content and derive the Skills face state.
pub fn parse_skills_str(content: &str) -> SkillSnapshotFile {
    let mut b = Builder::new();

    for line in content.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if entry.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let Some(msg) = entry.get("message") else {
            continue;
        };
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
        let ts = entry
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(iso_to_epoch_ms)
            .unwrap_or(0);

        if role == "user" {
            // A user message starts a new agent turn.
            b.turn_count += 1;
            b.in_turn = true;
            b.last_ts = ts;
            continue;
        }
        if role == "toolResult" {
            // Tool output returned: the agent is still mid-turn.
            b.in_turn = true;
            b.last_ts = ts;
            continue;
        }
        if role != "assistant" {
            continue;
        }

        let Some(blocks) = msg.get("content").and_then(Value::as_array) else {
            continue;
        };
        b.last_ts = ts;

        // A settled turn ends with a plain text/thinking assistant message.
        let mut has_tool_call = false;
        for block in blocks {
            if block.get("type").and_then(Value::as_str) != Some("toolCall") {
                continue;
            }
            has_tool_call = true;
            let name = block.get("name").and_then(Value::as_str).unwrap_or("");
            let args = block.get("arguments").cloned().unwrap_or(Value::Null);
            b.on_tool_call(name, &args, ts);
        }
        if !has_tool_call {
            b.in_turn = false;
        }
        b.last_assistant_had_tool_call = has_tool_call;
        b.entry_index += 1;
    }

    wrap(b.finish())
}

fn wrap(state: SkillState) -> SkillSnapshotFile {
    SkillSnapshotFile {
        version: 1,
        live: true,
        generated_at: String::new(),
        state: Some(state),
    }
}

struct Builder {
    active_skill: Option<String>,
    references: HashMap<String, SkillReference>,
    actions: Vec<SkillAction>,
    compliance: Vec<ComplianceCheck>,
    inspected: u64,
    modified: u64,
    start_ms: Option<u64>,
    last_ts: u64,
    turn_count: u64,
    in_turn: bool,
    /// True when the newest assistant message carried a tool call (mid-turn).
    last_assistant_had_tool_call: bool,
    entry_index: usize,
}

impl Builder {
    fn new() -> Self {
        Self {
            active_skill: None,
            references: HashMap::new(),
            actions: Vec::new(),
            compliance: Vec::new(),
            inspected: 0,
            modified: 0,
            start_ms: None,
            last_ts: 0,
            turn_count: 0,
            in_turn: false,
            last_assistant_had_tool_call: false,
            entry_index: 0,
        }
    }

    fn on_tool_call(&mut self, tool_name: &str, args: &Value, ts: u64) {
        if ts > 0 {
            if self.start_ms.is_none() {
                self.start_ms = Some(ts);
            }
            self.last_ts = ts;
        }
        let raw_path = args.get("path").and_then(Value::as_str).unwrap_or("");
        let norm = raw_path.replace('\\', "/");
        let base = norm.rsplit('/').find(|s| !s.is_empty()).unwrap_or("");

        // 1. Skill activation via SKILL.md read.
        if tool_name == "read" && norm.ends_with("SKILL.md") {
            self.active_skill = skill_name_from(&norm);
            self.push_action(
                "read",
                base_label(&norm),
                format!(
                    "Loaded skill entry point: {}",
                    self.active_skill.clone().unwrap_or_default()
                ),
                ts,
            );
            return;
        }

        // 2. Guidance reference document read.
        if tool_name == "read" && norm.contains("/references/") {
            self.references
                .entry(base.to_string())
                .or_insert(SkillReference {
                    name: base.to_string(),
                    summary: reference_summary(base).to_string(),
                });
            self.push_action(
                "read",
                base,
                format!("Guidance: {}", reference_summary(base)),
                ts,
            );
            return;
        }

        // 3. Engine docs consultation.
        if tool_name == "read" && norm.contains("@earendil-works/pi-coding-agent/docs") {
            self.push_action("doc_consult", base, format!("Engine API doc: {base}"), ts);
            return;
        }

        // 4. Plain file inspection.
        if tool_name == "read" && !raw_path.is_empty() {
            self.inspected += 1;
            self.push_action("read", base, format!("Inspecting {base}"), ts);
            return;
        }

        // 5. Modification → compliance gates.
        if tool_name == "edit" || tool_name == "write" {
            if !raw_path.is_empty() {
                self.modified += 1;
            }
            let payload = match tool_name {
                "write" => args
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                "edit" => args
                    .get("edits")
                    .and_then(Value::as_array)
                    .map(|edits| {
                        edits
                            .iter()
                            .filter_map(|e| e.get("newText").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default(),
                _ => String::new(),
            };
            run_gates(&norm, &payload, ts, &mut self.compliance);
            let verb = if tool_name == "edit" {
                "Modifying"
            } else {
                "Writing"
            };
            self.push_action(tool_name, base, format!("{verb} {base}"), ts);
            return;
        }

        // 6. Bash verification commands.
        if tool_name == "bash" {
            if let Some(cmd) = args.get("command").and_then(Value::as_str) {
                let target: String = cmd.chars().take(40).collect();
                let summary: String = cmd.chars().take(48).collect();
                self.push_action("bash", &target, format!("Shell: {summary}"), ts);
            }
        }
    }

    fn push_action(&mut self, kind: &str, target: &str, summary: String, ts: u64) {
        self.actions.push(SkillAction {
            kind: kind.to_string(),
            target: target.to_string(),
            summary,
            timestamp: ts,
        });
    }

    fn finish(self) -> SkillState {
        let start = self.start_ms.unwrap_or(self.last_ts);
        SkillState {
            active_skill: self.active_skill,
            references: self.references.into_values().collect(),
            actions: {
                let mut a = self.actions;
                if a.len() > MAX_ACTIONS {
                    a.drain(..a.len() - MAX_ACTIONS);
                }
                a
            },
            compliance: self.compliance,
            inspected_count: self.inspected,
            modified_count: self.modified,
            start_time: start,
            last_update_time: self.last_ts,
            in_turn: self.in_turn,
            turn_count: self.turn_count,
        }
    }
}

/// Skill name: the path segment directly before `SKILL.md`.
fn skill_name_from(norm_path: &str) -> Option<String> {
    let segments: Vec<&str> = norm_path.split('/').filter(|s| !s.is_empty()).collect();
    let idx = segments.iter().position(|s| *s == "SKILL.md")?;
    if idx > 0 {
        Some(segments[idx - 1].to_string())
    } else {
        Some("pi-skill".to_string())
    }
}

fn base_label(norm_path: &str) -> &str {
    norm_path.rsplit('/').next().unwrap_or(norm_path)
}

/// Guidance summary heuristics, ported from pi-plugin-dev's tracker.
fn reference_summary(filename: &str) -> &'static str {
    let lower = filename.to_lowercase();
    if lower.contains("command-completion") {
        "Trailing Space Contract & Autocomplete Engine"
    } else if lower.contains("tools-and-schema") {
        "StringEnum Rule & Tool Execution Contract"
    } else if lower.contains("lifecycle") {
        "Lifecycle Events & Engine Compatibility"
    } else if lower.contains("state-persistence") {
        "Branch-Aware Session State & Global Config"
    } else if lower.contains("api-docs") {
        "Live Local Engine Documentation Index"
    } else {
        "Skill Guidance Reference Document"
    }
}

/// Compliance gates over a mutated file. Pure function of (path, code) so the
/// Skills face can score the same invariants the TS auditor enforces.
fn run_gates(path: &str, code: &str, _ts: u64, out: &mut Vec<ComplianceCheck>) {
    if code.is_empty() && !path.ends_with("package.json") {
        return;
    }
    let code = strip_comments(code);
    let push = |check: ComplianceCheck, out: &mut Vec<ComplianceCheck>| {
        if let Some(existing) = out
            .iter_mut()
            .find(|c| c.rule == check.rule && c.label == check.label)
        {
            *existing = check;
        } else {
            out.push(check);
        }
    };

    // package.json gates: peer-deps + manifest hygiene.
    if path.ends_with("package.json") {
        if let Ok(v) = serde_json::from_str::<Value>(&code) {
            let core_in_deps = v
                .get("dependencies")
                .and_then(Value::as_object)
                .map(|d| d.keys().any(|k| k.starts_with("@earendil-works")))
                .unwrap_or(false);
            if core_in_deps {
                push(ComplianceCheck {
                    rule: "peer-deps".into(),
                    label: "Peer Dependencies".into(),
                    status: "fail".into(),
                    details: "Core @earendil-works package in dependencies — must stay in peerDependencies.".into(),
                    ..Default::default()
                }, out);
            } else if v.get("dependencies").is_some() {
                push(
                    ComplianceCheck {
                        rule: "peer-deps".into(),
                        label: "Peer Dependencies".into(),
                        status: "pass".into(),
                        details: "No core package bundled in dependencies".into(),
                        ..Default::default()
                    },
                    out,
                );
            }
            if v.get("type").and_then(Value::as_str) != Some("module") {
                push(
                    ComplianceCheck {
                        rule: "manifest-hygiene".into(),
                        label: "ESM Manifest".into(),
                        status: "warn".into(),
                        details: "package.json missing \"type\": \"module\".".into(),
                        ..Default::default()
                    },
                    out,
                );
            }
            return;
        }
    }

    // StringEnum schema rule.
    let has_union = code.contains("Type.Union") && code.contains("Type.Literal");
    let has_string_enum = code.contains("StringEnum(");
    if code.contains("registerTool") || code.contains("Type.Object") {
        let status = if has_union {
            "fail"
        } else if has_string_enum {
            "pass"
        } else {
            return;
        };
        push(
            ComplianceCheck {
                rule: "string-enum".into(),
                label: "StringEnum Schema Rule".into(),
                status: status.into(),
                details: if has_union {
                    "Type.Union of Type.Literal breaks Google Gemini — use StringEnum([...])."
                        .into()
                } else {
                    "StringEnum used for enum parameters (Gemini compatible).".into()
                },
                ..Default::default()
            },
            out,
        );
    }

    // Error reporting contract: tool errors must throw, not return isError.
    if code.contains("registerTool") && code.contains("execute") {
        let returns_error = code.contains("isError: true")
            || code.contains("error: \"")
            || code.contains("error: '");
        let throws = code.contains("throw new Error(");
        if returns_error && !throws {
            push(ComplianceCheck {
                rule: "error-throw".into(),
                label: "Error Reporting Contract".into(),
                status: "warn".into(),
                details: "Tool returns an error payload instead of throwing Error (isError is only set via throw).".into(),
                ..Default::default()
            }, out);
        } else if throws {
            push(
                ComplianceCheck {
                    rule: "error-throw".into(),
                    label: "Error Reporting Contract".into(),
                    status: "pass".into(),
                    details: "Tool errors reported via throw new Error().".into(),
                    ..Default::default()
                },
                out,
            );
        }
    }

    // Trailing Space Contract: markers belong in label/description, not value.
    for line in code.lines() {
        let trimmed = line.trim_start();
        if let Some(pos) = trimmed.find("value:") {
            let rest = &trimmed[pos..];
            if rest.contains('✓') || rest.contains('●') || rest.contains('○') {
                push(ComplianceCheck {
                    rule: "trailing-space".into(),
                    label: "Completion Marker Guard".into(),
                    status: "fail".into(),
                    details: "Marker found inside item.value — value is inserted verbatim; markers belong in label/description.".into(),
                    ..Default::default()
                }, out);
                break;
            }
        }
    }

    // Lifecycle cleanup: every pi.on must be tracked for unsubscribe.
    if code.contains("pi.on(") {
        let status = if code.contains("unsubscribe")
            || code.contains("track(")
            || code.contains("unsubscribers")
        {
            "pass"
        } else {
            "warn"
        };
        push(
            ComplianceCheck {
                rule: "lifecycle-cleanup".into(),
                label: "Lifecycle Listener Cleanup".into(),
                status: status.into(),
                details: if status == "pass" {
                    "Listeners tracked for session_shutdown unsubscribe.".into()
                } else {
                    "pi.on() result not stored — listener leaks on /reload.".into()
                },
                ..Default::default()
            },
            out,
        );
    }

    // UI-mode guard: TUI-only state must be guarded.
    if code.contains("custom(") {
        let status = if code.contains("hasUI") || code.contains("canOverlay") {
            "pass"
        } else {
            "warn"
        };
        push(
            ComplianceCheck {
                rule: "ui-mode-guard".into(),
                label: "UI Mode Guard".into(),
                status: status.into(),
                details: if status == "pass" {
                    "TUI state guarded by hasUI/canOverlay.".into()
                } else {
                    "custom() without a hasUI guard breaks headless sessions.".into()
                },
                ..Default::default()
            },
            out,
        );
    }
}

/// Comment-blind scan: removes `//` line comments and `/* */` blocks while
/// respecting string literals (a `//` inside `"https://…"` is content).
fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                // Line comment: skip to end of line.
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                while let Some(n) = chars.next() {
                    if n == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        break;
                    }
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    out
}

/// RFC3339 UTC timestamp (`2026-09-23T21:02:02.995Z`) → epoch milliseconds.
fn iso_to_epoch_ms(s: &str) -> Option<u64> {
    let s = s.trim();
    let bytes = s.as_bytes();
    if bytes.len() < 19 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    let num = |from: usize, to: usize| s.get(from..to)?.parse::<u64>().ok();
    let year = num(0, 4)? as i64;
    let month = num(5, 7)?;
    let day = num(8, 10)?;
    let hour = num(11, 13)?;
    let min = num(14, 16)?;
    let sec = num(17, 19)?;
    let ms = num(20, 23).unwrap_or(0);

    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hour as i64 * 3600 + min as i64 * 60 + sec as i64;
    if secs < 0 {
        return None;
    }
    Some((secs as u64) * 1000 + ms)
}

/// Howard Hinnant's `days_from_civil` (proleptic Gregorian).
fn days_from_civil(y: i64, m: u64, d: u64) -> i64 {
    let y = y - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((m + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let f = parse_skills_str(SAMPLE);
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
        assert!(s.in_turn == false, "final assistant text message = settled");
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

    #[test]
    #[ignore]
    fn parses_real_session_skills() {
        let path = std::env::var("PI_SIDEBAR_SESSION_FILE").expect("set env var");
        let f = parse_session_skills(Path::new(&path)).expect("parse");
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
}
