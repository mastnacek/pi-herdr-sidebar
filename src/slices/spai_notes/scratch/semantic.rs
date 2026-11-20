//! Semantic filter (`Ctrl+R`): on **Enter** — `spai search` (meaning + words,
//! all projects) in a background thread with the same spinner the Settings tab
//! shows. Never automatic.
//!
//! The result is the *allow-set keyed by file path* — stable across buffer
//! edits (indices shift the moment a line is added or removed, which made the
//! filtered list visibly shift).
use crate::shared::spai_client;
use super::super::state::SpaiNotesState;
use super::state::{SemanticJob, SemanticMessage};
use std::sync::mpsc;

impl SpaiNotesState {
    /// Starts the semantic search for `query` in a background thread.
    /// Progress arrives through `scratch.poll_semantic()` (tick-driven).
    pub fn scratch_semantic_filter(&mut self, query: &str) {
        if query.trim().len() < 3 {
            self.scratch.last_summary = Some("⚠ Dotaz je příliš krátký (min. 3 znaky)".to_string());
            return;
        }
        let Some(proj) = self
            .projects
            .get(self.selected_project_idx)
            .or_else(|| self.projects.first())
        else {
            self.scratch.last_summary = Some("⚠ Žádný projekt s docs/spai".to_string());
            return;
        };
        let (cfg, path, query) = (self.core.clone(), proj.path.clone(), query.to_string());
        let (tx, rx) = mpsc::channel::<SemanticMessage>();
        self.scratch.semantic_job = Some(SemanticJob {
            receiver: rx,
            progress: super::state::SemanticProgress::Running {
                step: 0,
                total: 1,
                label: "Sémantické hledání…".to_string(),
            },
            spinner_tick: 0,
        });
        std::thread::spawn(move || {
            let _ = tx.send(SemanticMessage::Progress {
                step: 0,
                total: 1,
                label: format!("spai search „{}“", query),
            });
            let msg = match spai_client::search_paths(&cfg, &path, &query) {
                Ok(allowed) => SemanticMessage::Done {
                    label: format!("Sémantický filtr: {} shod (spai search)", allowed.len()),
                    allowed,
                },
                Err(e) => SemanticMessage::Error(e),
            };
            let _ = tx.send(msg);
        });
    }
}
