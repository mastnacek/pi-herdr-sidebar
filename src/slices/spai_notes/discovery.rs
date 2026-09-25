//! Discovery of projects with docs/spai or .pi/spai folders across workspace.
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use super::note::{parse_spai_markdown, SpaiNoteItem};

#[derive(Debug, Clone, Deserialize)]
pub struct CachedProject {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectsCache {
    #[serde(default)]
    pub projects: Vec<CachedProject>,
}

#[derive(Debug, Clone)]
pub struct SpaiProjectSummary {
    pub name: String,
    pub path: PathBuf,
    pub spai_dir: PathBuf,
    pub items: Vec<SpaiNoteItem>,
}

impl SpaiProjectSummary {
    pub fn new(name: String, path: PathBuf, spai_dir: PathBuf) -> Self {
        Self {
            name,
            path,
            spai_dir,
            items: Vec::new(),
        }
    }

    pub fn scan_items(&mut self) {
        self.items.clear();
        if !self.spai_dir.exists() {
            return;
        }

        if let Ok(entries) = fs::read_dir(&self.spai_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");

                    if file_name.ends_with(".md") && !file_name.starts_with('.') {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Some(item) = parse_spai_markdown(&content, path) {
                                self.items.push(item);
                            }
                        }
                    }
                }
            }
        }

        // Sort items by timestamp or ID descending (newest first)
        self.items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }
}

/// Discovers candidate SPAI directory inside a project path.
pub fn find_spai_dir(project_path: &Path) -> Option<PathBuf> {
    let docs_spai = project_path.join("docs").join("spai");
    if docs_spai.is_dir() {
        return Some(docs_spai);
    }
    let dot_spai = project_path.join(".pi").join("spai");
    if dot_spai.is_dir() {
        return Some(dot_spai);
    }
    None
}

/// Locates ~/.pi/agent dir.
fn get_pi_agent_dir() -> Option<PathBuf> {
    dirs_home().map(|h| h.join(".pi").join("agent"))
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Discovers all projects that have a SPAI directory.
pub fn discover_spai_projects(current_project_cwd: Option<&Path>) -> Vec<SpaiProjectSummary> {
    let mut projects: Vec<SpaiProjectSummary> = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    // 1. Current workspace project (if provided)
    if let Some(cwd) = current_project_cwd {
        if let Some(spai_dir) = find_spai_dir(cwd) {
            let name = cwd
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("Current")
                .to_string();
            let mut summary = SpaiProjectSummary::new(name, cwd.to_path_buf(), spai_dir);
            summary.scan_items();
            seen_paths.insert(cwd.to_path_buf());
            projects.push(summary);
        }
    }

    // 2. Load from pi-projects-cache.json
    if let Some(agent_dir) = get_pi_agent_dir() {
        let cache_file = agent_dir.join("pi-projects-cache.json");
        if let Ok(content) = fs::read_to_string(&cache_file) {
            if let Ok(cache) = serde_json::from_str::<ProjectsCache>(&content) {
                for p in cache.projects {
                    let pb = PathBuf::from(&p.path);
                    if seen_paths.contains(&pb) {
                        continue;
                    }
                    if let Some(spai_dir) = find_spai_dir(&pb) {
                        seen_paths.insert(pb.clone());
                        let mut summary = SpaiProjectSummary::new(p.name, pb, spai_dir);
                        summary.scan_items();
                        if !summary.items.is_empty() {
                            projects.push(summary);
                        }
                    }
                }
            }
        }
    }

    projects
}
