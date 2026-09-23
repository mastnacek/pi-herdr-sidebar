use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HerdrPaneInfo {
    pub pane_id: String,
    pub tab_id: Option<String>,
    pub workspace_id: Option<String>,
    pub label: Option<String>,
    pub cwd: Option<String>,
    pub focused: Option<bool>,
    pub agent: Option<String>,
    pub terminal_title: Option<String>,
    #[serde(rename = "agentSession")]
    pub agent_session: Option<serde_json::Value>,
}

impl HerdrPaneInfo {
    /// Extract the pi session id from `agent_session` (shape:
    /// `{"agent":"pi","kind":"id","value":"01a0cf71-..."}`).
    pub fn session_id(&self) -> Option<String> {
        self.agent_session
            .as_ref()
            .and_then(|s| s.get("value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnvelopePanes {
    panes: Option<Vec<HerdrPaneInfo>>,
    pane: Option<HerdrPaneInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HerdrEnvelope {
    result: Option<EnvelopePanes>,
}

pub struct HerdrClient {
    pub bin_path: String,
}

impl HerdrClient {
    pub fn new() -> Self {
        let bin_path = env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".to_string());
        Self { bin_path }
    }

    #[allow(dead_code)]
    pub fn run_json(&self, args: &[&str]) -> Result<Value, String> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.args(args);
        cmd.arg("--json");

        let output = cmd.output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&stdout).map_err(|e| e.to_string())
    }

    pub fn notify(&self, title: &str, body: &str) {
        let _ = Command::new(&self.bin_path)
            .args(["notification", "show", title, "--body", body])
            .status();
    }

    pub fn list_panes(&self) -> Vec<HerdrPaneInfo> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.args(["pane", "list"]);

        let output = match cmd.output() {
            Ok(o) if o.status.success() => o.stdout,
            _ => return Vec::new(),
        };

        let stdout = String::from_utf8_lossy(&output);
        let envelope: Result<HerdrEnvelope, _> = serde_json::from_str(&stdout);
        if let Ok(env) = envelope {
            if let Some(res) = env.result {
                return res.panes.unwrap_or_default();
            }
        }
        Vec::new()
    }

    pub fn find_sidebar_pane(&self, tab_id: Option<&str>) -> Option<HerdrPaneInfo> {
        let panes = self.list_panes();
        panes.into_iter().find(|p| {
            let label = p.label.as_deref().unwrap_or("");
            let is_sidebar = label.eq_ignore_ascii_case("pi-sidebar")
                || label.eq_ignore_ascii_case("Pi Sidebar")
                || label.eq_ignore_ascii_case("pi-herdr-sidebar")
                || label.eq_ignore_ascii_case("Pi Herdr Sidebar");
            if let Some(tid) = tab_id {
                is_sidebar && p.tab_id.as_deref() == Some(tid)
            } else {
                is_sidebar
            }
        })
    }

    pub fn open_plugin_pane(&self, entrypoint: &str) -> Result<(), String> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.args([
            "plugin",
            "pane",
            "open",
            "--plugin",
            "pi.herdr-sidebar",
            "--entrypoint",
            entrypoint,
            "--placement",
            "split",
            "--direction",
            "right",
            "--no-focus",
        ]);
        let output = cmd.output().map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    #[allow(dead_code)]
    pub fn split_right(&self, cwd: Option<&str>, ratio: f32) -> Result<String, String> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.args([
            "pane",
            "split",
            "--current",
            "--direction",
            "right",
            "--no-focus",
        ]);
        let ratio_str = format!("{:.2}", ratio);
        cmd.args(["--ratio", &ratio_str]);

        if let Some(dir) = cwd {
            cmd.args(["--cwd", dir]);
        }

        let output = cmd.output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(err);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let env: Result<HerdrEnvelope, _> = serde_json::from_str(&stdout);
        if let Ok(e) = env {
            if let Some(r) = e.result {
                if let Some(p) = r.pane {
                    return Ok(p.pane_id);
                }
            }
        }

        Ok("unknown".to_string())
    }

    #[allow(dead_code)]
    pub fn run_in_pane(&self, pane_id: &str, command: &[&str]) -> Result<(), String> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.args(["pane", "run", pane_id]);
        cmd.args(command);
        let output = cmd.output().map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    pub fn close_pane(&self, pane_id: &str) -> Result<(), String> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.args(["pane", "close", pane_id]);
        let output = cmd.output().map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    #[allow(dead_code)]
    pub fn rename_pane(&self, pane_id: &str, label: &str) -> Result<(), String> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.args(["pane", "rename", pane_id, label]);
        let _ = cmd.status();
        Ok(())
    }
}
