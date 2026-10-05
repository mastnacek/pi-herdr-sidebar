//! Actions on duplicate / similar notes detected during Smart Note creation.
use super::note::SpaiStatus;
use super::similarity::find_similar_notes;
use super::state::SpaiNotesState;
use super::storage_format::format_spai_markdown;

impl SpaiNotesState {
    /// Returns the top similar matching note for the currently typed input (if any).
    pub fn top_similar_match(&self) -> Option<String> {
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
        item.body
            .push_str(&format!("\n\n- [{}] Aktualizace: {}", ts, text_to_append));

        let formatted = format_spai_markdown(item);
        std::fs::write(&item.file_path, formatted)
            .map_err(|e| format!("Chyba při zápisu: {}", e))?;

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

        let next_status = item.status.next_cycle();
        item.status = next_status;
        let formatted = format_spai_markdown(item);
        std::fs::write(&item.file_path, formatted)
            .map_err(|e| format!("Chyba při zápisu: {}", e))?;

        self.status_message = Some(format!(
            "Změněn stav {} na {}",
            target_id,
            next_status.as_str()
        ));
        Ok(next_status)
    }
}
