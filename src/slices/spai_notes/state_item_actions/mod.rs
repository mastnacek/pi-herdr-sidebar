//! Note items lifecycle and status cycling actions.
use super::note::SpaiStatus;
use super::state::SpaiNotesState;

impl SpaiNotesState {
    /// Cycles the status of the selected record through `spai set` (it keeps the
    /// rest of the file and the index in step).
    pub fn cycle_selected_status(&mut self) -> Result<(), String> {
        let proj = self
            .projects
            .get(self.selected_project_idx)
            .ok_or_else(|| "Není vybrán žádný projekt".to_string())?;
        let item = proj
            .items
            .get(self.selected_item_idx)
            .ok_or_else(|| "Není vybrána žádná položka".to_string())?;
        let next = item.status.next_cycle();
        let (path, id) = (proj.path.clone(), item.id.clone());
        let file = item.file_path.clone();
        self.set_status_via_spai(&path, &id, &file, next)?;
        self.status_message = Some(format!("Stav změněn: {} {}", next.glyph(), next.as_str()));
        Ok(())
    }

    /// `spai set` for one record, then the record is re-read from its file.
    /// Returns the new status. Nothing changes in memory when `spai` fails.
    pub fn set_status_via_spai(
        &mut self,
        project: &std::path::Path,
        id: &str,
        file: &std::path::Path,
        status: SpaiStatus,
    ) -> Result<SpaiStatus, String> {
        crate::shared::spai_client::set_status(&self.core, project, id, status.as_str())?;
        let content = std::fs::read_to_string(file).map_err(|e| format!("Čtení selhalo: {e}"))?;
        let fresh = super::storage_format::parse_spai_markdown(&content, file.to_path_buf())
            .ok_or_else(|| "Soubor nelze načíst jako SPAI poznámku".to_string())?;
        for p in self.projects.iter_mut().filter(|p| p.path == project) {
            if let Some(it) = p.items.iter_mut().find(|i| i.file_path == file) {
                *it = fresh.clone();
            }
        }
        Ok(fresh.status)
    }

    /// Arm the two-step delete confirmation for the selected item.
    pub fn begin_delete_selected(&mut self) {
        if self.current_items().is_empty() {
            self.status_message = Some("Žádná položka ke smazání".to_string());
            return;
        }
        self.delete_confirm_active = true;
        self.status_message = Some(
            "Smazat položku trvale (spai delete, bez návratu)? Delete = potvrdit, jiná klávesa = zrušit"
                .to_string(),
        );
    }

    /// Disarm the delete confirmation without deleting anything.
    pub fn cancel_delete(&mut self) {
        self.delete_confirm_active = false;
        self.status_message = None;
    }

    /// Permanently delete the selected record with `spai delete` (file, index,
    /// database; its id is never reused) and refresh. Nothing changes in memory
    /// when `spai` fails. There is no undo of a delete.
    pub fn confirm_delete_selected(&mut self) -> Result<(), String> {
        self.delete_confirm_active = false;

        let proj = self
            .projects
            .get_mut(self.selected_project_idx)
            .ok_or_else(|| "Není vybrán žádný projekt".to_string())?;
        proj.ensure_items();

        if self.selected_item_idx >= proj.items.len() {
            return Err("Není vybrána žádná položka".to_string());
        }

        let (path, id) = (proj.path.clone(), proj.items[self.selected_item_idx].id.clone());
        crate::shared::spai_client::delete_record(&self.core, &path, &id)
            .map_err(|e| format!("Smazání selhalo: {e}"))?;
        let item = proj.items.remove(self.selected_item_idx);

        let title = item.title.clone();
        if self.selected_item_idx >= proj.items.len() {
            self.selected_item_idx = proj.items.len().saturating_sub(1);
        }
        self.viewer_scroll = 0;
        let cwd = self.current_project_path.clone();
        self.refresh(cwd.as_deref(), true);
        self.status_message = Some(format!("Smazáno (spai delete, bez návratu): {}", title));
        Ok(())
    }
}

#[cfg(test)]
mod tests;
