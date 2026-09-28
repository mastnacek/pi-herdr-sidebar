//! Attribution of prompt sections to the files pi reads, and drift detection.
//!
//! `SYSTEM.md` and `APPEND_SYSTEM.md` live either in the trusted project's
//! `.pi/` directory or in the agent directory; pi prefers the project file and
//! does not combine the two. The transcript is the ground truth for *whether*
//! a section exists — matching its text against the candidates only names the
//! source.
//!
//! Pi caches discovered resources at session start and only re-reads them on
//! `/reload` (`docs/slash-commands.md`), so a source file whose mtime is newer
//! than the system message that carried its text proves the prompt is stale.
//! Only *loaded* files are ever compared — an unloaded candidate is silence,
//! not noise.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::{PromptSource, SourceState};
use crate::shared::dirs_home;

/// Resolve a section's text against the well-known files pi reads, in the
/// precedence order pi uses (project `.pi/` first, then the agent directory).
///
/// `None` when the section is absent; `Inline` when it exists but matches no
/// candidate (joined sources or an extension).
pub(super) fn attribute(
    section: Option<&String>,
    cwd: &str,
    file_name: &str,
) -> Option<PromptSource> {
    let text = section?;
    let home = dirs_home();
    let source = match_source(text, &candidate_paths(cwd, file_name, home.as_deref()));
    Some(source.unwrap_or(PromptSource::Inline))
}

/// `SYSTEM.md` / `APPEND_SYSTEM.md` locations: project first, then the agent
/// directory, then the prompt files kept under `agents/`.
///
/// The last group exists because `--append-system-prompt <path>` never goes
/// through discovery, and the documented alias pattern keeps those files in
/// `~/.pi/agent/agents/`. Content matching keeps it evidence rather than a
/// guess: a file only wins when its text *is* the section body.
pub(super) fn candidate_paths(cwd: &str, file_name: &str, home: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if !cwd.is_empty() {
        paths.push(Path::new(cwd).join(".pi").join(file_name));
    }
    if let Some(home) = home {
        paths.push(home.join(".pi").join("agent").join(file_name));
        paths.extend(agent_prompt_files(home));
    }
    paths
}

/// The two canonical `APPEND_SYSTEM.md` locations that *discovery* would read,
/// in pi's precedence order — no `agents/` files, no content matching.
///
/// Used to explain an absence: if one of these exists while the engine sent no
/// `addendum`, something other than "there is no file" is going on (an untrusted
/// project, or a file written after the session was built).
pub(super) fn canonical_append_path(cwd: &str, home: Option<&Path>) -> Option<String> {
    let mut candidates = Vec::new();
    if !cwd.is_empty() {
        candidates.push(Path::new(cwd).join(".pi").join("APPEND_SYSTEM.md"));
    }
    if let Some(home) = home {
        candidates.push(home.join(".pi").join("agent").join("APPEND_SYSTEM.md"));
    }
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| path.display().to_string())
}

/// Every `*.md` under `<home>/.pi/agent/agents`, sorted for determinism.
fn agent_prompt_files(home: &Path) -> Vec<PathBuf> {
    let dir = home.join(".pi").join("agent").join("agents");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file() && path.extension().is_some_and(|extension| extension == "md")
        })
        .collect();
    files.sort();
    files
}

/// Match a section body against candidate files. Pi joins multiple append
/// sources with a blank line, and a CLI flag can be prepended, so an exact
/// match or a suffix match both count.
pub(super) fn match_source(text: &str, candidates: &[PathBuf]) -> Option<PromptSource> {
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

/// Drift of one loaded file against the moment its text entered the prompt.
pub(super) fn file_state(path: &str, loaded_ms: u64) -> SourceState {
    let Ok(meta) = std::fs::metadata(expand_path(path)) else {
        return SourceState::Missing;
    };
    let Ok(mtime) = meta.modified() else {
        return SourceState::Ok;
    };
    let ms = mtime
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    // A second of slack absorbs the write/replay race on the first request.
    if loaded_ms > 0 && ms > loaded_ms.saturating_add(1_000) {
        SourceState::Modified
    } else {
        SourceState::Ok
    }
}

/// Expand a leading `~` so `project_context` paths can be stat-ed.
fn expand_path(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~') {
        if let Some(home) = dirs_home() {
            return home.join(rest.trim_start_matches(['/', '\\']));
        }
    }
    PathBuf::from(path)
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}

/// Parse the session's `YYYY-MM-DDTHH:MM:SS(.sss)Z` timestamps to epoch
/// milliseconds. Returns `0` on anything unexpected, which disables drift
/// detection rather than producing a false warning.
pub(super) fn iso_to_epoch_ms(iso: &str) -> u64 {
    let (date, rest) = match iso.split_once('T') {
        Some(parts) => parts,
        None => return 0,
    };
    let mut date_parts = date.split('-');
    let (Some(y), Some(m), Some(d)) = (
        date_parts.next().and_then(|v| v.parse::<i64>().ok()),
        date_parts.next().and_then(|v| v.parse::<u32>().ok()),
        date_parts.next().and_then(|v| v.parse::<u32>().ok()),
    ) else {
        return 0;
    };

    let time = rest.trim_end_matches('Z');
    let mut time_parts = time.split(':');
    let (Some(hh), Some(mm)) = (
        time_parts.next().and_then(|v| v.parse::<u64>().ok()),
        time_parts.next().and_then(|v| v.parse::<u64>().ok()),
    ) else {
        return 0;
    };
    let (ss, millis) = match time_parts.next() {
        Some(sec) => match sec.split_once('.') {
            Some((s, frac)) => (
                s.parse::<u64>().unwrap_or(0),
                frac.chars()
                    .take(3)
                    .collect::<String>()
                    .parse::<u64>()
                    .unwrap_or(0),
            ),
            None => (sec.parse::<u64>().unwrap_or(0), 0),
        },
        None => (0, 0),
    };

    let days = days_from_civil(y, m, d);
    if days < 0 {
        return 0;
    }
    (((days as u64) * 86_400 + hh * 3_600 + mm * 60 + ss) * 1_000).saturating_add(millis)
}

/// Days since 1970-01-01 (Howard Hinnant's civil-from-days, inverted).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
