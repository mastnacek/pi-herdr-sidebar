//! Code Mode detection for a Claude Code pane.
//!
//! Code Mode (`CLAUDE_CODE_EXPERIMENTAL_CODE_MODE=1`) makes the model drive
//! tools through one internal `REPL` tool. Two sources tell whether it is on:
//!
//! 1. **Transcript** — the real state of the running session. Claude Code
//!    writes `{"type":"repl-mode","replMode":true,"sessionId":…}` lines when
//!    the session has the REPL tool; sessions without Code Mode have no such
//!    line. `tool_use` blocks named `REPL` confirm it was actually used.
//! 2. **Settings** — `env.CLAUDE_CODE_EXPERIMENTAL_CODE_MODE` in
//!    `~/.claude/settings.json`: what a *new* session will get. Fallback only.

use std::fs;

/// Where the "Code Mode is on" verdict came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CodeMode {
    #[default]
    Off,
    /// Only the global setting says so; the session may predate it.
    Config,
    /// The session transcript proves it.
    Transcript,
}

impl CodeMode {
    pub fn is_on(self) -> bool {
        self != CodeMode::Off
    }
}

/// True for the transcript line `{"type":"repl-mode","replMode":true}`.
pub fn is_repl_mode_line(kind: &str, repl_mode: Option<bool>) -> bool {
    kind == "repl-mode" && repl_mode == Some(true)
}

/// True when settings JSON has `env.CLAUDE_CODE_EXPERIMENTAL_CODE_MODE` set to
/// `1`/`true` (string or bool). Reads the `env` object only.
pub fn settings_enable_code_mode(settings_json: &str) -> bool {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(settings_json) else {
        return false;
    };
    match v.get("env").and_then(|e| e.get("CLAUDE_CODE_EXPERIMENTAL_CODE_MODE")) {
        Some(serde_json::Value::String(s)) => matches!(s.trim(), "1" | "true"),
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::Number(n)) => n.as_u64() == Some(1),
        _ => false,
    }
}

/// Reads `~/.claude/settings.json`; any failure counts as "off".
pub fn user_settings_enable_code_mode() -> bool {
    let Some(home) = crate::shared::dirs_home() else {
        return false;
    };
    fs::read_to_string(home.join(".claude").join("settings.json"))
        .map(|s| settings_enable_code_mode(&s))
        .unwrap_or(false)
}

/// Verdict for a transcript, falling back to the setting when it shows none.
pub fn resolve(from_transcript: bool, setting_on: bool) -> CodeMode {
    match (from_transcript, setting_on) {
        (true, _) => CodeMode::Transcript,
        (false, true) => CodeMode::Config,
        (false, false) => CodeMode::Off,
    }
}

#[cfg(test)]
mod tests;
