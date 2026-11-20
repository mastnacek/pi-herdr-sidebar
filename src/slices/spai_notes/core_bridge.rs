//! Bridge to the `spai` binary: the one place where the sidebar writes
//! records, checks duplicates, changes status and searches. `spai` owns the
//! rules (English storage, `title_cs`, thresholds, `pending` when offline);
//! the sidebar starts it as a process (argv, never a shell) with `--json` and
//! reads stdout plus the exit code (0 ok, 1 failure, 2 usage, 3 duplicate).
//! Everything here blocks (process, network inside `spai`): the UI calls it
//! through [`spawn`] so the UI thread never waits.
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// Shown when no `spai` binary is found; browsing records still works.
pub const NOT_FOUND: &str =
    "spai nenalezen — nainstaluj `cargo install --path cli` v repu spai nebo nastav SPAI_BIN";

/// What a finished `spai` call left behind.
#[derive(Debug, Clone)]
pub struct RunOut {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Starts `spai` with `args` in `cwd`. Tests replace it with a fake.
pub trait Runner: Send + Sync {
    fn run(&self, args: &[String], cwd: &Path) -> Result<RunOut, String>;
}

/// The real thing: a child process.
pub struct ProcessRunner {
    /// `None` when the binary was not found (every call then fails with [`NOT_FOUND`]).
    pub bin: Option<PathBuf>,
    /// Extra environment for the child (tests redirect paths here).
    pub env: Vec<(String, String)>,
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "spai.exe"
    } else {
        "spai"
    }
}

/// `SPAI_BIN`, then `PATH`, then `~/.cargo/bin`.
fn find_binary() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SPAI_BIN").map(PathBuf::from) {
        if p.is_file() {
            return Some(p);
        }
    }
    let on_path = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|v| std::env::split_paths(&v).collect::<Vec<_>>())
        .map(|d| d.join(exe_name()))
        .find(|p| p.is_file());
    on_path.or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        Some(PathBuf::from(home).join(".cargo").join("bin").join(exe_name())).filter(|p| p.is_file())
    })
}

impl Runner for ProcessRunner {
    fn run(&self, args: &[String], cwd: &Path) -> Result<RunOut, String> {
        let bin = self.bin.as_ref().ok_or_else(|| NOT_FOUND.to_string())?;
        let mut cmd = Command::new(bin);
        cmd.args(args).current_dir(cwd);
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        let out = cmd
            .output()
            .map_err(|e| format!("spai se nespustil: {e}"))?;
        Ok(RunOut {
            code: out.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

#[derive(Clone)]
pub struct CoreConfig {
    pub runner: Arc<dyn Runner>,
}

impl CoreConfig {
    pub fn from_process() -> Self {
        // Tests never start the real `spai` (it would touch the real backlogs).
        #[cfg(test)]
        return super::core_bridge::emulator::config();
        #[cfg(not(test))]
        Self {
            runner: Arc::new(ProcessRunner {
                bin: find_binary(),
                env: Vec::new(),
            }),
        }
    }
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self::from_process()
    }
}

impl std::fmt::Debug for CoreConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CoreConfig")
    }
}

/// A record similar to the typed text, from any project.
#[derive(Debug, Clone, PartialEq)]
pub struct SimilarHit {
    pub project: String,
    pub id: String,
    /// Czech title when the record has one, else its title.
    pub title: String,
    pub score: f32,
    pub path: String,
    /// At or above the duplicate threshold (else only related).
    pub duplicate: bool,
}

impl SimilarHit {
    fn from_json(v: &Value, duplicate: bool) -> Self {
        let text = |k: &str| v[k].as_str().unwrap_or_default().to_string();
        Self {
            project: text("project"),
            id: text("id"),
            title: text("title"),
            score: v["score"].as_f64().unwrap_or(0.0) as f32,
            path: text("path"),
            duplicate,
        }
    }

    pub fn percent(&self) -> u32 {
        (self.score * 100.0).round() as u32
    }
}

fn hits(data: &Value) -> Vec<SimilarHit> {
    let list = |key: &str, duplicate: bool| -> Vec<SimilarHit> {
        data[key]
            .as_array()
            .map(|a| a.iter().map(|v| SimilarHit::from_json(v, duplicate)).collect())
            .unwrap_or_default()
    };
    let mut all = list("duplicates", true);
    all.extend(list("related", false));
    all
}

/// A record `add` wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct Written {
    pub id: String,
    pub project: String,
    pub path: PathBuf,
    pub title_cs: Option<String>,
    pub title: String,
    /// Work still to do (no network): `translate_en`, `embed`, ...
    pub pending: Vec<String>,
    pub warnings: Vec<String>,
    pub related: Vec<SimilarHit>,
}

impl Written {
    /// One status line: id, Czech title, warning when the record waits for the network.
    pub fn notice(&self) -> String {
        let title = self.title_cs.as_deref().unwrap_or(&self.title);
        let mut s = format!("✓ {} · {}", self.id, title);
        if !self.pending.is_empty() {
            s.push_str(&format!(
                " · ⚠ bez sítě, čeká: {} (doplní spai reindex)",
                self.pending.join(", ")
            ));
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AddOutcome {
    Written(Written),
    /// Nothing was written: a similar record exists (duplicates first).
    Duplicates(Vec<SimilarHit>),
}

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

#[cfg(test)]
pub mod testkit;
#[cfg(test)]
pub mod emulator;
#[cfg(test)]
mod tests;
