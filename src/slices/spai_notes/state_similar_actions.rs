//! Duplicate-check actions for the creation dialog and the integrated editor:
//! `Ctrl+D` runs `spai similar` on demand (background), `Ctrl+O/A/U` act on
//! its results, `Ctrl+S` over a duplicate stopped by `add` saves anyway.
//! Every UI tick only drains an already finished background call.
use super::core_bridge::{self, SimilarHit};
use super::note::SpaiStatus;
use super::state::SpaiNotesState;
use super::storage_format::format_spai_markdown;

impl SpaiNotesState {
    /// Drains finished background calls (duplicate check, creation write) into
    /// the UI. Purely passive: no timer, no keystroke tracking.
    pub fn poll_dedup_receiver(&mut self) {
        self.poll_add_job();
        if let Some(rx) = &self.dedup.receiver {
            if let Ok(result) = rx.try_recv() {
                self.dedup.receiver = None;
                self.dedup.is_evaluating = false;
                match result {
                    Ok(hits) => self.dedup.matches = hits,
                    Err(e) => {
                        self.dedup.visible = false;
                        self.status_message = Some(format!("⚠ {e}"));
                    }
                }
            }
        }
    }

    /// Runs `spai similar` for `query` on demand (Ctrl+D) in the background.
    /// The panel is shown always; all projects are compared.
    pub fn run_dedup(&mut self, query: String) {
        self.dedup.visible = true;
        self.dedup.matches.clear();
        self.dedup.receiver = None;
        self.dedup.force_text = None;
        self.dedup.is_evaluating = false;
        if query.trim().len() < 3 {
            return;
        }
        let Some(proj) = self.projects.get(self.selected_project_idx) else {
            return;
        };
        let (cfg, path) = (self.core.clone(), proj.path.clone());
        self.dedup.receiver = Some(core_bridge::spawn(move || {
            core_bridge::similar(&cfg, &path, query.trim())
        }));
        self.dedup.is_evaluating = true;
    }

    /// Hides the dedup overlay (Esc over the panel).
    pub fn close_dedup_panel(&mut self) {
        self.dedup.visible = false;
        self.dedup.force_text = None;
    }

    /// The top dedup match (any project).
    pub fn top_similar_match(&self) -> Option<SimilarHit> {
        self.dedup.matches.first().cloned()
    }

    /// Project and item index of a hit, when the sidebar knows the record.
    fn locate(&mut self, hit: &SimilarHit) -> Option<(usize, usize)> {
        let pi = self.projects.iter().position(|p| p.name == hit.project)?;
        self.projects[pi].ensure_items();
        let ii = self.projects[pi].items.iter().position(|i| i.id == hit.id)?;
        Some((pi, ii))
    }

    /// Aborts creation dialog and opens the first similar matching note in edit mode.
    pub fn open_first_similar_match(&mut self) -> bool {
        let Some(hit) = self.top_similar_match() else {
            return false;
        };
        let Some((pi, ii)) = self.locate(&hit) else {
            self.status_message = Some(format!("Záznam {} ({}) nenalezen", hit.id, hit.project));
            return false;
        };
        self.selected_project_idx = pi;
        self.selected_item_idx = ii;
        self.close_creation_dialog();
        self.open_edit_dialog();
        self.status_message = Some(format!("Otevřen existující záznam {}", hit.id));
        true
    }

    /// Appends the newly typed text as a timestamped comment to the existing matched note.
    pub fn append_to_first_similar_match(&mut self) -> Result<String, String> {
        let text_to_append = self.creation_dialog.title_input.trim().to_string();
        if text_to_append.is_empty() {
            return Err("Text k připojení je prázdný".to_string());
        }
        let hit = self
            .top_similar_match()
            .ok_or_else(|| "Nebyl nalezen žádný podobný záznam k připojení".to_string())?;
        let (pi, ii) = self
            .locate(&hit)
            .ok_or_else(|| format!("Záznam {} nenalezen", hit.id))?;

        let (ts, _) = super::time_utils::current_timestamp_and_date();
        let mut updated = self.projects[pi].items[ii].clone();
        updated
            .body
            .push_str(&format!("\n\n- [{}] Aktualizace: {}", ts, text_to_append));
        super::note_io::write_atomic(&updated.file_path, &format_spai_markdown(&updated))
            .map_err(|e| format!("Chyba při zápisu: {}", e))?;
        self.projects[pi].items[ii].body = updated.body;

        self.close_creation_dialog();
        self.status_message = Some(format!("Připojeno k existujícímu {}", hit.id));
        Ok(hit.id)
    }

    /// Cycles the status of the first similar matching note (e.g. todo -> working -> done).
    pub fn cycle_status_of_first_similar_match(&mut self) -> Result<SpaiStatus, String> {
        let hit = self
            .top_similar_match()
            .ok_or_else(|| "Nebyl nalezen žádný podobný záznam".to_string())?;
        let (pi, ii) = self
            .locate(&hit)
            .ok_or_else(|| format!("Záznam {} nenalezen", hit.id))?;
        let item = &self.projects[pi].items[ii];
        let (next, file, path) = (
            item.status.next_cycle(),
            item.file_path.clone(),
            self.projects[pi].path.clone(),
        );
        let done = self.set_status_via_spai(&path, &hit.id, &file, next)?;
        self.status_message = Some(format!("Změněn stav {} na {}", hit.id, done.as_str()));
        Ok(done)
    }
}
