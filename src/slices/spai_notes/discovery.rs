//! Discovery of projects with docs/spai or .pi/spai folders across workspace.
//!
//! Discovery is intentionally **lazy**: it resolves the project list (name + path)
//! only. Note bodies are read on demand by [`SpaiProjectSummary::ensure_items`],
//! so opening the `@project` picker costs a couple of `stat()` calls per project
//! instead of reading and parsing every note of every project.
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::note::SpaiNoteItem;
use super::storage_format::parse_spai_markdown;

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
    items_loaded: bool,
}

impl SpaiProjectSummary {
    pub fn new(name: String, path: PathBuf, spai_dir: PathBuf) -> Self {
        Self {
            name,
            path,
            spai_dir,
            items: Vec::new(),
            items_loaded: false,
        }
    }

    /// Reads and parses the note files of this project, once.
    pub fn ensure_items(&mut self) {
        if !self.items_loaded {
            self.scan_items();
        }
    }

    fn scan_items(&mut self) {
        self.items_loaded = true;
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

/// Cheap "does this project have notes?" probe: lists the directory, reads no file.
pub fn dir_has_notes(spai_dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(spai_dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        let name = e.file_name();
        let name = name.to_string_lossy();
        name.ends_with(".md") && !name.starts_with('.')
    })
}

/// `(mtime, len)` of a path — the cheap change signal used to skip rescans.
pub fn file_fingerprint(path: &Path) -> Option<(u64, u64)> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos() as u64;
    Some((mtime, meta.len()))
}

/// Path of pi's project cache, the source of the project list.
pub fn projects_cache_path() -> Option<PathBuf> {
    dirs_home().map(|h| h.join(".pi").join("agent").join("pi-projects-cache.json"))
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
            let summary = SpaiProjectSummary::new(name, cwd.to_path_buf(), spai_dir);
            seen_paths.insert(cwd.to_path_buf());
            projects.push(summary);
        }
    }

    // 2. Load from pi-projects-cache.json
    if let Some(cache_file) = projects_cache_path() {
        if let Ok(content) = fs::read_to_string(&cache_file) {
            if let Ok(cache) = serde_json::from_str::<ProjectsCache>(&content) {
                for p in cache.projects {
                    let pb = PathBuf::from(&p.path);
                    if seen_paths.contains(&pb) {
                        continue;
                    }
                    if let Some(spai_dir) = find_spai_dir(&pb) {
                        seen_paths.insert(pb.clone());
                        if dir_has_notes(&spai_dir) {
                            projects.push(SpaiProjectSummary::new(p.name, pb, spai_dir));
                        }
                    }
                }
            }
        }
    }

    projects
}

#[cfg(test)]
mod tests;
