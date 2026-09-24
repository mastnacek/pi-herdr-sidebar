use std::path::Path;
use std::process::Command;

use super::GitTelemetry;

pub fn git_info(cwd: &Path) -> Option<GitTelemetry> {
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

    // 2. Latest commit details
    let log_out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["log", "-1", "--format=%h|%cr|%s"])
        .output()
        .ok();

    let mut commit_hash = String::new();
    let mut commit_age = String::new();
    let mut commit_msg = String::new();

    if let Some(lo) = log_out {
        if lo.status.success() {
            let log_line = String::from_utf8_lossy(&lo.stdout);
            let parts: Vec<&str> = log_line.trim().splitn(3, '|').collect();
            if parts.len() >= 3 {
                commit_hash = parts[0].to_string();
                commit_age = parts[1].to_string();
                commit_msg = parts[2].to_string();
            }
        }
    }

    Some(GitTelemetry {
        branch,
        ahead,
        behind,
        staged,
        unstaged,
        untracked,
        commit_hash,
        commit_msg,
        commit_age,
    })
}
