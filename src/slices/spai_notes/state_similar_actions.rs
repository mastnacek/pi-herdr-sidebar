//! Duplicate-check actions for the Smart Input dialog and the integrated
//! editor: `Ctrl+D` runs the check on demand, `Ctrl+O/A/U` act on its results,
//! and every UI tick only drains an already-finished background computation.
use super::note::SpaiStatus;
use super::similarity::{find_similar_notes, find_similar_notes_hybrid};
use super::state::SpaiNotesState;
use super::storage_format::format_spai_markdown;
use std::sync::mpsc::channel;

impl SpaiNotesState {
    /// Drains a finished background dedup computation into the panel. Purely
    /// passive: no timer, no keystroke tracking, no network call — the plan's
    /// "dedup na zkratku" guarantee.
    pub fn poll_dedup_receiver(&mut self) {
        if let Some(rx) = &self.dedup.receiver {
            if let Ok(matches) = rx.try_recv() {
                self.dedup.matches = matches;
                self.dedup.is_evaluating = false;
                self.dedup.receiver = None;
            }
        }
    }

    /// Runs the duplicate check for `query` on demand (Ctrl+D). With an API
    /// key it vectorizes in the background; without one it falls back to local
    /// text similarity + stored vectors, synchronously, without an error.
    /// The overlay panel is shown in both cases.
    pub fn run_dedup(&mut self, api_key: &str, model: &str, threshold: f64, query: String) {
        self.dedup.visible = true;
        self.dedup.matches.clear();
        self.dedup.receiver = None;

        let raw = query.trim().to_string();
        if raw.len() < 3 {
            return; // nothing meaningful to compare
        }

        let Some(proj) = self.projects.get(self.selected_project_idx) else {
            return;
        };

        if api_key.trim().is_empty() {
            // Local fallback: text similarity + stored vectors, no network.
            let stored_vectors =
                crate::slices::settings::load_project_vectors(&proj.path).map(|s| s.vectors);
            self.dedup.matches = find_similar_notes_hybrid(
                &raw,
                &proj.items,
                None,
                stored_vectors.as_ref(),
                threshold,
                4,
            );
            self.dedup.is_evaluating = false;
            return;
        }

        self.dedup.is_evaluating = true;
        let (tx, rx) = channel();
        self.dedup.receiver = Some(rx);

        let key = api_key.to_string();
        let emb_model = model.to_string();
        let items = proj.items.clone();
        let proj_path = proj.path.clone();

        std::thread::spawn(move || {
            let stored_vectors =
                crate::slices::settings::load_project_vectors(&proj_path).map(|s| s.vectors);

            let cand_vec = match crate::slices::settings::vector_service::request_embeddings(
                &key, &emb_model, &[&raw],
            ) {
                Ok(mut vecs) => vecs.pop(),
                Err(_) => None,
            };

            let matches = find_similar_notes_hybrid(
                &raw,
                &items,
                cand_vec.as_deref(),
                stored_vectors.as_ref(),
                threshold,
                4,
            );

            let _ = tx.send(matches);
        });
    }

    /// Hides the dedup overlay (Esc over the panel).
    pub fn close_dedup_panel(&mut self) {
        self.dedup.visible = false;
    }

    /// Returns the top dedup match, falling back to a fast local text check
    /// against the currently typed creation input.
    pub fn top_similar_match(&self) -> Option<String> {
        if !self.dedup.matches.is_empty() {
            return self.dedup.matches.first().map(|m| m.id.clone());
        }

        let raw = self.creation_dialog.title_input.trim();
        if raw.len() < 3 {
            return None;
        }
        let proj = self.projects.get(self.selected_project_idx)?;
        let matches = find_similar_notes(raw, &proj.items, 0.45, 1);
        matches.first().map(|m| m.id.clone())
    }

    /// Aborts creation dialog and opens the first similar matching note in edit mode.
    pub fn open_first_similar_match(&mut self) -> bool {
        let Some(target_id) = self.top_similar_match() else {
            return false;
        };

        if let Some(proj) = self.projects.get_mut(self.selected_project_idx) {
            if let Some(pos) = proj.items.iter().position(|it| it.id == target_id) {
                self.selected_item_idx = pos;
                self.close_creation_dialog();
                self.open_edit_dialog();
                self.status_message = Some(format!("Otevřen existující záznam {}", target_id));
                return true;
            }
        }
        false
    }

    /// Appends the newly typed text as a timestamped comment to the existing matched note.
    pub fn append_to_first_similar_match(&mut self) -> Result<String, String> {
        let text_to_append = self.creation_dialog.title_input.trim().to_string();
        if text_to_append.is_empty() {
            return Err("Text k připojení je prázdný".to_string());
        }

        let target_id = self
            .top_similar_match()
            .ok_or_else(|| "Nebyl nalezen žádný podobný záznam k připojení".to_string())?;

        let proj = self
            .projects
            .get_mut(self.selected_project_idx)
            .ok_or_else(|| "Projekt nenalezen".to_string())?;

        let item = proj
            .items
            .iter_mut()
            .find(|it| it.id == target_id)
            .ok_or_else(|| format!("Záznam {} nenalezen", target_id))?;

        let (ts, _) = super::time_utils::current_timestamp_and_date();
        let mut updated = item.clone();
        updated
            .body
            .push_str(&format!("\n\n- [{}] Aktualizace: {}", ts, text_to_append));

        let formatted = format_spai_markdown(&updated);
        super::note_io::write_atomic(&updated.file_path, &formatted)
            .map_err(|e| format!("Chyba při zápisu: {}", e))?;
        item.body = updated.body;

        self.close_creation_dialog();
        self.status_message = Some(format!("Připojeno k existujícímu {}", target_id));
        Ok(target_id)
    }

    /// Cycles the status of the first similar matching note (e.g. todo -> working -> done).
    pub fn cycle_status_of_first_similar_match(&mut self) -> Result<SpaiStatus, String> {
        let target_id = self
            .top_similar_match()
            .ok_or_else(|| "Nebyl nalezen žádný podobný záznam".to_string())?;

        let proj = self
            .projects
            .get_mut(self.selected_project_idx)
            .ok_or_else(|| "Projekt nenalezen".to_string())?;

        let item = proj
            .items
            .iter_mut()
            .find(|it| it.id == target_id)
            .ok_or_else(|| format!("Záznam {} nenalezen", target_id))?;

        // Write first, mutate only on success.
        let mut updated = item.clone();
        updated.status = updated.status.next_cycle();
        updated.symbol = updated.status.symbol().to_string();
        let formatted = format_spai_markdown(&updated);
        super::note_io::write_atomic(&updated.file_path, &formatted)
            .map_err(|e| format!("Chyba při zápisu: {}", e))?;
        let next_status = updated.status;
        item.status = next_status;
        item.symbol = updated.symbol;
        item.body = updated.body;

        self.status_message = Some(format!(
            "Změněn stav {} na {}",
            target_id,
            next_status.as_str()
        ));
        Ok(next_status)
    }
}
