//! The `spai` process: how it is found and started (argv, never a shell).
use std::path::{Path, PathBuf};
use std::process::Command;

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
pub(super) fn find_binary() -> Option<PathBuf> {
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
        Some(
            PathBuf::from(home)
                .join(".cargo")
                .join("bin")
                .join(exe_name()),
        )
        .filter(|p| p.is_file())
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
