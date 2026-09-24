use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct SpaiRecord {
    pub id: String,
    pub title: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SpaiIndex {
    #[serde(default)]
    pub version: u32,
    #[serde(default, rename = "lastUpdated")]
    pub last_updated: String,
    #[serde(default)]
    pub records: Vec<SpaiRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct SpaiCounts {
    pub done: usize,
    pub working: usize,
    pub waiting: usize,
    pub todo: usize,
    pub cancelled: usize,
    pub ideas: usize,
    pub notes: usize,
    pub total_tasks: usize,
    pub total_items: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SpaiTelemetry {
    pub file_path: Option<PathBuf>,
    pub index: SpaiIndex,
    pub counts: SpaiCounts,
}

impl SpaiCounts {
    pub fn from_records(records: &[SpaiRecord]) -> Self {
        let mut counts = Self::default();
        for r in records {
            let status = r.status.to_lowercase();
            let kind = r.kind.to_lowercase();

            if kind == "idea" || status == "idea" {
                counts.ideas += 1;
            } else if kind == "note" || status == "note" || status == "inbox" {
                counts.notes += 1;
            } else {
                match status.as_str() {
                    "done" => counts.done += 1,
                    "working" => counts.working += 1,
                    "waiting" => counts.waiting += 1,
                    "cancelled" => counts.cancelled += 1,
                    "todo" | _ => counts.todo += 1,
                }
            }
        }
        counts.total_tasks =
            counts.done + counts.working + counts.waiting + counts.todo + counts.cancelled;
        counts.total_items = counts.total_tasks + counts.ideas + counts.notes;
        counts
    }
}

/// Find `docs/spai/.index.json` by checking cwd, parent directories, or project root.
pub fn find_spai_index_path(cwd: Option<&str>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(c) = cwd {
        let p = Path::new(c);
        candidates.push(p.join("docs").join("spai").join(".index.json"));

        // Walk up parents (up to 4 levels) in case of monorepo subprojects
        let mut curr = p.parent();
        let mut depth = 0;
        while let Some(parent) = curr {
            if depth >= 4 {
                break;
            }
            candidates.push(parent.join("docs").join("spai").join(".index.json"));
            curr = parent.parent();
            depth += 1;
        }
    }

    // Check fallback in current process working directory
    candidates.push(PathBuf::from("docs/spai/.index.json"));

    for candidate in candidates {
        if candidate.is_file() {
            // Check if it's readable and not completely empty
            if let Ok(meta) = fs::metadata(&candidate) {
                if meta.len() > 0 {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

/// Parse SPAI index file into telemetry directly without external plugins.
pub fn parse_spai_index(path: &Path) -> Option<SpaiTelemetry> {
    let content = fs::read_to_string(path).ok()?;
    let index: SpaiIndex = serde_json::from_str(&content).ok()?;
    let counts = SpaiCounts::from_records(&index.records);

    Some(SpaiTelemetry {
        file_path: Some(path.to_path_buf()),
        index,
        counts,
    })
}
