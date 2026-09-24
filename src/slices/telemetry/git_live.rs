use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{GitCommitLog, GitTelemetry, RepoTelemetry};

const NESTED_SCAN_DEPTH: usize = 2;

fn recent_commits(repo_root: &Path, count: usize) -> Vec<GitCommitLog> {
    let Some(out) = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["log", &format!("-{}", count), "--format=%h|%cr|%s"])
        .output()
        .ok()
    else {
        return Vec::new();
    };

    if !out.status.success() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut commits = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.trim().splitn(3, '|').collect();
        if parts.len() >= 3 {
            commits.push(GitCommitLog {
                hash: parts[0].to_string(),
                age: parts[1].to_string(),
                message: parts[2].to_string(),
            });
        }
    }
    commits
}

fn repo_toplevel(cwd: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

fn has_git_dir(dir: &Path) -> bool {
    dir.join(".git").exists()
}

fn repo_display_name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| root.display().to_string())
}

/// Extract edited file paths from session JSONL content (edit/write tool calls).
fn extract_edited_paths(session_content: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in session_content.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("message") {
            continue;
        }
        let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) else {
            continue;
        };
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) != Some("toolCall") {
                continue;
            }
            let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if !matches!(name, "edit" | "write") {
                continue;
            }
            let args = block.get("arguments");
            let path = args
                .and_then(|a| a.get("path"))
                .or_else(|| args.and_then(|a| a.get("file_path")))
                .and_then(|p| p.as_str());
            if let Some(p) = path {
                paths.push(p.to_string());
            }
        }
    }
    paths
}

/// Walk up from a file path to the nearest directory containing `.git`.
fn nearest_repo_root(file_path: &str) -> Option<PathBuf> {
    let p = Path::new(file_path);
    let mut dir = p.parent()?;
    loop {
        if has_git_dir(dir) {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

/// Scan immediate (up to 2-deep) subdirectories for nested git repos.
fn scan_nested_repos(cwd: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(cwd) else {
        return found;
    };
    for entry in entries.flatten() {
        let sub = entry.path();
        if !sub.is_dir() {
            continue;
        }
        if has_git_dir(&sub) {
            found.push(sub);
        } else if NESTED_SCAN_DEPTH > 1 {
            if let Ok(sub_entries) = std::fs::read_dir(&sub) {
                for sub_entry in sub_entries.flatten() {
                    let deeper = sub_entry.path();
                    if deeper.is_dir() && has_git_dir(&deeper) {
                        found.push(deeper);
                    }
                }
            }
        }
    }
    found
}

/// Monorepo fallback: cwd itself is not a git repo. Discover nested repos
/// via the session edit trail (edit/write paths), falling back to a shallow
/// subdirectory scan. Per-repo branch summary + recent commits.
pub fn build_monorepo_telemetry(cwd: &Path, session_content: &str) -> GitTelemetry {
    let mut repo_files: BTreeMap<PathBuf, u32> = BTreeMap::new();

    for path in extract_edited_paths(session_content) {
        if let Some(root) = nearest_repo_root(&path) {
            *repo_files.entry(root).or_insert(0) += 1;
        }
    }

    let mut repos: Vec<RepoTelemetry> = Vec::new();

    if !repo_files.is_empty() {
        for (root, count) in repo_files {
            let commits = recent_commits(&root, 2);
            repos.push(RepoTelemetry {
                name: repo_display_name(&root),
                root: root.clone(),
                touched_files: count,
                recent_commits: commits,
            });
        }
    } else {
        for root in scan_nested_repos(cwd).into_iter().take(4) {
            let commits = recent_commits(&root, 2);
            if commits.is_empty() {
                continue;
            }
            repos.push(RepoTelemetry {
                name: repo_display_name(&root),
                root: root.clone(),
                touched_files: 0,
                recent_commits: commits,
            });
        }
    }

    repos.sort_by(|a, b| b.touched_files.cmp(&a.touched_files));

    GitTelemetry {
        branch: String::new(),
        ahead: 0,
        behind: 0,
        staged: 0,
        unstaged: 0,
        untracked: 0,
        recent_commits: Vec::new(),
        touched_repos: repos,
    }
}

pub fn git_info(cwd: &Path) -> Option<GitTelemetry> {
    let toplevel = repo_toplevel(cwd)?;
    let cwd_is_root = toplevel == cwd;

    // 1. Status with branch info in porcelain format
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["status", "--porcelain=v2", "--branch"])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let status_str = String::from_utf8_lossy(&out.stdout);
    let mut branch = String::new();
    let mut ahead = 0u32;
    let mut behind = 0u32;
    let mut staged = 0u32;
    let mut unstaged = 0u32;
    let mut untracked = 0u32;

    for line in status_str.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            branch = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            let mut parts = rest.split_whitespace();
            if let Some(p) = parts.next() {
                if let Some(n) = p.strip_prefix('+').and_then(|s| s.parse::<u32>().ok()) {
                    ahead = n;
                }
            }
            if let Some(p) = parts.next() {
                if let Some(n) = p.strip_prefix('-').and_then(|s| s.parse::<u32>().ok()) {
                    behind = n;
                }
            }
        } else if line.starts_with("1 ") || line.starts_with("2 ") {
            let bytes = line.as_bytes();
            if bytes.len() >= 4 {
                let x = bytes[2];
                let y = bytes[3];
                if x != b'.' {
                    staged += 1;
                }
                if y != b'.' {
                    unstaged += 1;
                }
            }
        } else if line.starts_with("? ") {
            untracked += 1;
        } else if line.starts_with("u ") {
            staged += 1;
            unstaged += 1;
        }
    }

    // 2. Recent commits protocol (last 3 commits)
    let recent_commits = recent_commits(cwd, 3);

    let _ = cwd_is_root;
    Some(GitTelemetry {
        branch,
        ahead,
        behind,
        staged,
        unstaged,
        untracked,
        recent_commits,
        touched_repos: Vec::new(),
    })
}
