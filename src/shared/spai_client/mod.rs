//! Client of the `spai` binary: the one place where the sidebar writes
//! records, checks duplicates, changes status and searches. `spai` owns the
//! rules (English storage, `title_cs`, thresholds, `pending` when offline);
//! the sidebar starts it as a process (argv, never a shell) with `--json` and
//! reads stdout plus the exit code (0 ok, 1 failure, 2 usage, 3 duplicate).
//! Everything here blocks (process, network inside `spai`): the UI calls it
//! through [`spawn`] so the UI thread never waits.
//!
//! - [`runner`] — the process (`Runner`, `ProcessRunner`, binary lookup)
//! - [`config`] — `CoreConfig`, what a slice holds to call `spai`
//! - [`reply`] — parsed answers (`SimilarHit`, `Written`, `AddOutcome`)
mod config;
mod reply;
mod runner;
#[cfg(test)]
pub mod testkit;
#[cfg(test)]
mod tests;

pub use config::CoreConfig;
pub use reply::{AddOutcome, SimilarHit, Written};
pub use runner::Runner;
#[cfg(test)]
pub use runner::{ProcessRunner, RunOut, NOT_FOUND};

use reply::hits;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

/// Runs `spai <args> --json` and returns the parsed `data` plus the exit code
/// and the warnings of stderr. A failure becomes the message of `spai`.
fn call(cfg: &CoreConfig, args: &[&str], cwd: &Path) -> Result<(i32, Value, Vec<String>), String> {
    let mut argv: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    argv.push("--json".to_string());
    let out = cfg.runner.run(&argv, cwd)?;
    let json: Value = serde_json::from_str(out.stdout.trim()).unwrap_or(Value::Null);
    if out.code != 0 && out.code != 3 {
        let msg = json["error"]
            .as_str()
            .map(String::from)
            .or_else(|| out.stderr.lines().rev().find(|l| !l.trim().is_empty()).map(String::from))
            .unwrap_or_else(|| format!("spai skončil kódem {}", out.code));
        return Err(msg);
    }
    let warnings = out
        .stderr
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pozor:"))
        .map(|l| l.trim().to_string())
        .collect();
    Ok((out.code, json["data"].clone(), warnings))
}

/// `spai add` into `project`. `force` writes although a duplicate exists.
pub fn add_record(
    cfg: &CoreConfig,
    project: &Path,
    text: &str,
    force: bool,
) -> Result<AddOutcome, String> {
    let proj = project.to_string_lossy();
    let mut args = vec!["add", text, "--project", &proj, "--source", "herdr-sidebar"];
    if force {
        args.push("--force");
    }
    let (code, data, warnings) = call(cfg, &args, project)?;
    if code == 3 || data["written"].as_bool() != Some(true) {
        return Ok(AddOutcome::Duplicates(hits(&data)));
    }
    let text = |k: &str| data[k].as_str().unwrap_or_default().to_string();
    Ok(AddOutcome::Written(Written {
        id: text("id"),
        project: text("project"),
        path: PathBuf::from(text("path")),
        title_cs: data["title_cs"].as_str().map(String::from),
        title: text("title"),
        pending: data["pending"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        warnings,
        related: hits(&data).into_iter().filter(|h| !h.duplicate).collect(),
    }))
}

/// `spai similar`: duplicates and related records of `text`, nothing is written.
pub fn similar(cfg: &CoreConfig, project: &Path, text: &str) -> Result<Vec<SimilarHit>, String> {
    let (_, data, _) = call(cfg, &["similar", text], project)?;
    Ok(hits(&data))
}

/// `spai set <id> <status>` in `project` (status name or symbol).
pub fn set_status(cfg: &CoreConfig, project: &Path, id: &str, status: &str) -> Result<(), String> {
    let proj = project.to_string_lossy();
    call(cfg, &["set", id, status, "--project", &proj], project).map(|_| ())
}

/// `spai search` over all projects: the files of the best hits.
pub fn search_paths(cfg: &CoreConfig, project: &Path, query: &str) -> Result<HashSet<PathBuf>, String> {
    let (_, data, _) = call(cfg, &["search", query, "--limit", "50"], project)?;
    Ok(data["results"]
        .as_array()
        .map(|a| a.iter().filter_map(|h| h["path"].as_str().map(PathBuf::from)).collect())
        .unwrap_or_default())
}

/// `spai reindex --all`: one line about what was done.
pub fn reindex_all(cfg: &CoreConfig, cwd: &Path) -> Result<String, String> {
    let (_, data, warnings) = call(cfg, &["reindex", "--all"], cwd)?;
    let t = &data["totals"];
    let n = |k: &str| t[k].as_u64().unwrap_or(0);
    let mut s = format!(
        "spai reindex: {} záznamů, vektory +{}, překlady +{}",
        n("records"),
        n("embedded"),
        n("translated")
    );
    if n("errors") > 0 {
        s.push_str(&format!(", chyb {}", n("errors")));
    }
    if let Some(w) = warnings.first() {
        s.push_str(&format!(" ({w})"));
    }
    Ok(s)
}

/// Records with a vector in all projects (`spai projects`).
pub fn vector_total(cfg: &CoreConfig, cwd: &Path) -> Result<usize, String> {
    let (_, data, _) = call(cfg, &["projects"], cwd)?;
    Ok(data["projects"]
        .as_array()
        .map(|a| a.iter().map(|p| p["with_vector"].as_u64().unwrap_or(0) as usize).sum())
        .unwrap_or(0))
}

/// Runs `job` on a new thread; the result arrives on the receiver (polled on the UI tick).
pub fn spawn<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static) -> Receiver<T> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(job());
    });
    rx
}
