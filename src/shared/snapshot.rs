use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabHit {
    pub id: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneSnapshot {
    pub version: u32,
    #[serde(default = "default_true")]
    pub live: bool,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub width: u16,
    #[serde(default, rename = "generatedAt")]
    pub generated_at: String,
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default, rename = "tabHits")]
    pub tab_hits: Option<Vec<TabHit>>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickRequest {
    pub at: u64,
    pub column: u16,
    pub row: u16,
}

impl PaneSnapshot {
    /// Read and parse snapshot from file. Tolerates transient partial writes.
    pub fn read_from_file<P: AsRef<Path>>(path: P) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

/// Request path for tab clicks: `<snapshot_path>.request.json`.
pub fn request_path<P: AsRef<Path>>(snapshot_path: P) -> PathBuf {
    let mut path = snapshot_path.as_ref().to_path_buf();
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sidebar.json");
    path.set_file_name(format!("{}.request.json", file_name));
    path
}

/// Write click or tab change request to file for Pi extension to consume.
pub fn write_tab_request<P: AsRef<Path>>(snapshot_path: P, column: u16, row: u16) -> bool {
    let req_path = request_path(snapshot_path);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let req = ClickRequest {
        at: now,
        column,
        row,
    };

    if let Ok(json) = serde_json::to_string(&req) {
        fs::write(req_path, json).is_ok()
    } else {
        false
    }
}

/// PID lock file guard for single-instance per pane.
pub struct PidLock {
    pub path: PathBuf,
}

impl PidLock {
    pub fn acquire<P: AsRef<Path>>(snapshot_path: P) -> Option<Self> {
        let mut pid_path = snapshot_path.as_ref().to_path_buf();
        let file_name = pid_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("sidebar.json");
        pid_path.set_file_name(format!("{}.pid", file_name));

        let current_pid = process::id();
        if let Ok(content) = fs::read_to_string(&pid_path) {
            let prev_pid = content.trim().parse::<u32>().ok();
            if let Some(pid) = prev_pid {
                if pid != current_pid && is_process_running(pid) {
                    // Another instance is already running for this snapshot
                    return None;
                }
            }
        }

        let _ = fs::write(&pid_path, current_pid.to_string());
        Some(Self { path: pid_path })
    }
}

impl Drop for PidLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn is_process_running(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use std::process::Command;
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .output();
        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            text.contains(&pid.to_string())
        } else {
            false
        }
    }
    #[cfg(unix)]
    {
        let res = unsafe { libc::kill(pid as i32, 0) };
        res == 0
    }
}
