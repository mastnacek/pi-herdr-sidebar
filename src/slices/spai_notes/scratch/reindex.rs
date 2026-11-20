//! After an edited Czech title: `spai reindex --project` in the background
//! (once per save, not per keystroke) so `retitle_en` is resolved at once when
//! the network is there. Failure is only a warning; the record keeps `pending`.
use super::super::state::SpaiNotesState;
use crate::shared::spai_client::{self, CoreConfig};
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;

/// `spai reindex --project <project> --json`; `Err` is the warning to show.
pub fn reindex_project(cfg: &CoreConfig, project: &Path) -> Result<(), String> {
    let argv = vec![
        "reindex".to_string(),
        "--project".to_string(),
        project.to_string_lossy().into_owned(),
        "--json".to_string(),
    ];
    let out = cfg.runner.run(&argv, project)?;
    let json: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap_or_default();
    let errors = json["data"]["totals"]["errors"].as_u64().unwrap_or(0);
    if out.code != 0 || errors > 0 {
        let why = json["error"].as_str().unwrap_or("záznam zůstává s pending");
        return Err(format!("spai reindex selhal ({why})"));
    }
    Ok(())
}

impl SpaiNotesState {
    /// Starts the background reindex of every project in `projects`.
    pub(super) fn start_reindex(&mut self, mut projects: Vec<PathBuf>) {
        projects.sort();
        projects.dedup();
        if projects.is_empty() {
            return;
        }
        let cfg = self.core.clone();
        self.scratch.reindex_job = Some(spai_client::spawn(move || {
            projects
                .iter()
                .filter_map(|p| reindex_project(&cfg, p).err())
                .collect()
        }));
    }

    /// Takes the warnings of a finished background reindex. Passive (tick).
    pub fn poll_reindex(&mut self) {
        let Some(rx) = &self.scratch.reindex_job else {
            return;
        };
        match rx.try_recv() {
            Ok(warnings) => {
                self.scratch.reindex_job = None;
                if let Some(w) = warnings.first() {
                    self.scratch.last_summary = Some(format!("⚠ {w}"));
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => self.scratch.reindex_job = None,
        }
    }

    /// Test helper: waits for the background reindex and returns its warnings.
    #[cfg(test)]
    pub fn wait_reindex(&mut self) -> Vec<String> {
        match self.scratch.reindex_job.take() {
            Some(rx) => rx.recv().unwrap_or_default(),
            None => Vec::new(),
        }
    }
}
