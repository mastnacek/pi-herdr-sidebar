use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Structured skill state written by the TS pi-sidebar extension as
/// `<pane>.skills.json` next to the pane snapshot. Source of truth is the
/// pi-plugin-dev event bus; this file makes it readable by the native sidebar.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSnapshotFile {
    #[serde(default)]
    pub version: u32,
    #[serde(default = "default_true")]
    pub live: bool,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub state: Option<SkillState>,
}

fn default_true() -> bool {
    true
}

/// Live skill tracking state as published by pi-plugin-dev.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillState {
    #[serde(default)]
    pub active_skill: Option<String>,
    #[serde(default)]
    pub references: Vec<SkillReference>,
    #[serde(default)]
    pub actions: Vec<SkillAction>,
    #[serde(default)]
    pub compliance: Vec<ComplianceCheck>,
    #[serde(default)]
    pub inspected_count: u64,
    #[serde(default)]
    pub modified_count: u64,
    #[serde(default)]
    pub start_time: u64,
    #[serde(default)]
    pub last_update_time: u64,
    #[serde(default)]
    pub in_turn: bool,
    #[serde(default)]
    pub turn_count: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SkillReference {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillAction {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub timestamp: u64,
}

/// One Gates entry: a compliance rule checked against the agent's mutations.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplianceCheck {
    #[serde(default)]
    pub rule: String,
    #[serde(default)]
    pub label: String,
    /// `pass` | `warn` | `fail`
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub details: String,
}

impl SkillSnapshotFile {
    /// Read and parse the skills sidecar. Tolerates transient partial writes.
    pub fn read_from_file<P: AsRef<Path>>(path: P) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Gates scorecard: `(passed, total)`.
    pub fn gate_score(&self) -> (usize, usize) {
        match &self.state {
            None => (0, 0),
            Some(s) => {
                let passed = s.compliance.iter().filter(|c| c.status == "pass").count();
                (passed, s.compliance.len())
            }
        }
    }
}

/// `[PASS]` / `[WARN]` / `[FAIL]` badge for a check status.
pub fn gate_badge(status: &str) -> &'static str {
    match status {
        "pass" => "PASS",
        "fail" => "FAIL",
        _ => "WARN",
    }
}

/// Action icon mirroring the TS skills panel.
pub fn action_icon(kind: &str) -> &'static str {
    match kind {
        "bash" => "🧪",
        "edit" => "✏️",
        "write" => "✍️",
        "doc_consult" => "📚",
        _ => "📖",
    }
}

/// Elapsed label mirroring the TS `elapsedLabel` (`4m55s` / `12.3s`).
pub fn elapsed_label(ms: u64) -> String {
    let seconds = ms / 1000;
    if seconds < 60 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let minutes = seconds / 60;
        let rest = seconds % 60;
        format!("{}m{:02}s", minutes, rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_skill_snapshot() {
        let raw = r#"{
            "version": 1,
            "live": true,
            "generatedAt": "2026-09-23T22:00:00.000Z",
            "state": {
                "live": true,
                "activeSkill": "herdr-plugin-dev",
                "references": [{"name": "SKILL.md", "summary": "Main guide"}],
                "actions": [{"type": "edit", "target": "src/main.rs", "summary": "Fix", "timestamp": 1}],
                "compliance": [
                    {"rule": "r1", "label": "Check A", "status": "pass", "details": "ok"},
                    {"rule": "r2", "label": "Check B", "status": "fail", "details": "bad"}
                ],
                "inspectedCount": 13,
                "modifiedCount": 4,
                "startTime": 1000,
                "lastUpdateTime": 2000,
                "inTurn": false,
                "turnCount": 50
            }
        }"#;
        let f: SkillSnapshotFile = serde_json::from_str(raw).unwrap();
        let s = f.state.as_ref().unwrap();
        assert_eq!(s.active_skill.as_deref(), Some("herdr-plugin-dev"));
        assert_eq!(s.turn_count, 50);
        assert_eq!(s.inspected_count, 13);
        assert_eq!(s.compliance.len(), 2);
        let (passed, total) = f.gate_score();
        assert_eq!((passed, total), (1, 2));
        assert_eq!(gate_badge("pass"), "PASS");
        assert_eq!(gate_badge("weird"), "WARN");
        assert_eq!(elapsed_label(295_000), "4m55s");
    }
}
