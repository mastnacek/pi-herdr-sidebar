//! Writing from the creation dialog: `spai add` runs in the background, the
//! dialog shows "ukládám…", a duplicate opens the panel ("uložit i tak" =
//! Ctrl+S there), a written record is selected in the list.
use super::core_bridge::{self, AddOutcome};
use super::dialog_state::DedupPanel;
use super::state::SpaiNotesState;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

/// One running `spai add` of the creation dialog.
#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct AddJob {
    pub rx: Receiver<Result<AddOutcome, String>>,
    /// The text sent (kept for "uložit i tak").
    pub text: String,
}

impl SpaiNotesState {
    /// Starts `spai add` for the selected project; `force` writes despite a duplicate.
    pub fn start_creation_add(&mut self, text: String, force: bool) -> Result<(), String> {
        if self.add_job.is_some() {
            return Err("Ukládání už běží".to_string());
        }
        let project = self
            .projects
            .get(self.selected_project_idx)
            .map(|p| p.path.clone())
            .ok_or_else(|| "Není vybrán žádný projekt".to_string())?;
        let (cfg, sent) = (self.core.clone(), text.clone());
        let rx = core_bridge::spawn(move || core_bridge::add_record(&cfg, &project, &sent, force));
        self.dedup = DedupPanel::default();
        self.creation_dialog.saving = true;
        self.status_message = Some("⏳ ukládám…".to_string());
        self.add_job = Some(AddJob { rx, text });
        Ok(())
    }

    /// Waits for the background write (tests only).
    #[cfg(test)]
    pub fn wait_add_job(&mut self) {
        while self.add_job.is_some() {
            self.poll_add_job();
            std::thread::yield_now();
        }
    }

    /// Ctrl+S over the duplicate panel: write the stopped text anyway.
    pub fn force_creation_add(&mut self) -> Result<(), String> {
        let text = self
            .dedup
            .force_text
            .clone()
            .ok_or_else(|| "Není co uložit i tak".to_string())?;
        self.start_creation_add(text, true)
    }

    /// Takes the finished background write, if any. Passive: called from the tick.
    pub fn poll_add_job(&mut self) {
        let Some(job) = &self.add_job else { return };
        let result = match job.rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("Zápis se nedokončil".to_string()),
        };
        let Some(job) = self.add_job.take() else { return };
        self.creation_dialog.saving = false;
        match result {
            Ok(AddOutcome::Written(w)) => {
                self.close_creation_dialog();
                self.reload_written(std::slice::from_ref(&w.path));
                self.select_record(&w.project, &w.id);
                let mut msg = w.notice();
                for warning in &w.warnings {
                    msg.push_str(&format!(" · {warning}"));
                }
                self.status_message = Some(msg);
            }
            Ok(AddOutcome::Duplicates(hits)) => {
                self.dedup = DedupPanel {
                    visible: true,
                    matches: hits,
                    force_text: Some(job.text),
                    ..DedupPanel::default()
                };
                self.status_message = Some("⚠ Podobný záznam už existuje — nic nezapsáno".to_string());
            }
            Err(e) => self.status_message = Some(format!("⚠ {e}")),
        }
    }

    /// Asks `spai projects` for the vector count in the background (never in
    /// `new`: tests and the first frame must not start a process).
    pub fn request_vector_count(&mut self) {
        if self.vector_rx.is_some() {
            return;
        }
        let cwd = self
            .current_project_path
            .clone()
            .or_else(|| self.projects.first().map(|p| p.path.clone()));
        let Some(cwd) = cwd else { return };
        self.vectors_stale = false;
        let cfg = self.core.clone();
        self.vector_rx = Some(core_bridge::spawn(move || core_bridge::vector_total(&cfg, &cwd)));
    }

    pub(super) fn poll_vector_total(&mut self) {
        if let Some(rx) = &self.vector_rx {
            if let Ok(r) = rx.try_recv() {
                self.vector_rx = None;
                if let Ok(n) = r {
                    self.vector_total = n;
                }
            }
        }
    }

    /// Re-reads the projects that hold the written files (new projects: full refresh).
    pub fn reload_written(&mut self, files: &[PathBuf]) {
        let mut unknown = false;
        for file in files {
            let dir = file.parent();
            match self.projects.iter_mut().find(|p| Some(p.spai_dir.as_path()) == dir) {
                Some(p) => p.reload_items(),
                None => unknown = true,
            }
        }
        if unknown {
            let cwd = self.current_project_path.clone();
            self.refresh(cwd.as_deref(), true);
        }
    }

    /// Selects record `id` of project `name` in the Notes list.
    pub fn select_record(&mut self, name: &str, id: &str) {
        let Some(pi) = self.projects.iter().position(|p| p.name == name) else {
            return;
        };
        self.projects[pi].ensure_items();
        if let Some(ii) = self.projects[pi].items.iter().position(|i| i.id == id) {
            self.selected_project_idx = pi;
            self.selected_item_idx = ii;
            self.viewer_scroll = 0;
        }
    }
}
