//! Attribution of prompt sections to the files pi reads.
//!
//! `SYSTEM.md` and `APPEND_SYSTEM.md` live either in the trusted project's
//! `.pi/` directory or in the agent directory; pi prefers the project file and
//! does not combine the two. The transcript is the ground truth for *whether*
//! a section exists — matching its text against the candidates only names the
//! source, and also surfaces a file that is present but was not loaded.

use std::path::{Path, PathBuf};

use super::PromptSource;
use crate::shared::dirs_home;

/// Resolve a section's text against the well-known files pi reads, in the
/// precedence order pi uses (project `.pi/` first, then the agent directory).
///
/// Returns the attribution plus every candidate that exists on disk, so a
/// file that is present but was *not* loaded (untrusted project, shadowed by
/// the other scope) still shows up in the tree.
pub(super) fn attribute(
    section: Option<&String>,
    cwd: &str,
    file_name: &str,
) -> (Option<PromptSource>, Vec<String>) {
    let candidates = candidate_paths(cwd, file_name);
    let present: Vec<String> = candidates
        .iter()
        .filter(|p| p.is_file())
        .map(|p| p.display().to_string())
        .collect();

    let Some(text) = section else {
        return (None, present);
    };
    let source = match_source(text, &candidates).unwrap_or(PromptSource::Inline);
    (Some(source), present)
}

/// `SYSTEM.md` / `APPEND_SYSTEM.md` locations, project first.
fn candidate_paths(cwd: &str, file_name: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if !cwd.is_empty() {
        paths.push(Path::new(cwd).join(".pi").join(file_name));
    }
    if let Some(home) = dirs_home() {
        paths.push(home.join(".pi").join("agent").join(file_name));
    }
    paths
}

/// Match a section body against candidate files. Pi joins multiple append
/// sources with a blank line, and a CLI flag can be prepended, so an exact
/// match or a suffix match both count.
fn match_source(text: &str, candidates: &[PathBuf]) -> Option<PromptSource> {
    let target = normalize(text);
    if target.is_empty() {
        return None;
    }
    for candidate in candidates {
        let Ok(raw) = std::fs::read_to_string(candidate) else {
            continue;
        };
        let file = normalize(&raw);
        if !file.is_empty() && (target == file || target.ends_with(&file)) {
            return Some(PromptSource::File(candidate.display().to_string()));
        }
    }
    None
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}
