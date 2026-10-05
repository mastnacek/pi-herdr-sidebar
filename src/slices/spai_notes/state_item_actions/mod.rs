//! Note items lifecycle and status cycling actions.
use super::note::{SpaiStatus, SpaiType};
use super::note_io::write_atomic;
use super::state::SpaiNotesState;
use super::storage_format::{format_spai_markdown, update_body_status_prefix};

impl SpaiNotesState {
    pub fn cycle_selected_status(&mut self) -> Result<(), String> {
        let proj = self
            .projects
            .get_mut(self.selected_project_idx)
            .ok_or_else(|| "Není vybrán žádný projekt".to_string())?;
        proj.ensure_items();

        let item = proj
            .items
            .get(self.selected_item_idx)
            .ok_or_else(|| "Není vybrána žádná položka".to_string())?;

        // Write first, mutate state only after the disk write succeeded, so
        // the UI never shows a status that does not exist on disk.
        let mut updated = item.clone();
        updated.status = updated.status.next_cycle();
        updated.symbol = updated.status.symbol().to_string();
        updated.body = update_body_status_prefix(&updated.body, updated.status);
        let updated_content = format_spai_markdown(&updated);
        write_atomic(&updated.file_path, &updated_content)?;

        let (glyph, status_str) = (updated.status.glyph(), updated.status.as_str());
        let item = proj
            .items
            .get_mut(self.selected_item_idx)
            .ok_or_else(|| "Není vybrána žádná položka".to_string())?;
        item.status = updated.status;
        item.symbol = updated.symbol;
        item.body = updated.body;

        self.status_message = Some(format!("Stav změněn: {} {}", glyph, status_str));

        Ok(())
    }

    pub fn create_quick_note_with_status(
        &mut self,
        _title: &str,
        _kind: SpaiType,
        status: SpaiStatus,
        body: &str,
    ) -> Result<String, String> {
        let proj = self
            .projects
            .get_mut(self.selected_project_idx)
            .ok_or_else(|| "Není vybrán žádný projekt".to_string())?;
        proj.ensure_items();

        // Delegate to the shared note_writer (disk-backed ids, create_new,
        // dir creation). The caller passes `body` as the prefix line
        // `"<glyph> <title>"`, so the writer's prefix detection resolves the
        // same kind/status the dialog selected.
        let written = super::note_writer::write_record(&proj, body.trim())?;
        debug_assert_eq!(written.item.status, status, "glyph/status mismatch");

        let id = written.id.clone();
        proj.items.insert(0, written.item);
        self.selected_item_idx = 0;
        self.viewer_scroll = 0;

        Ok(id)
    }

    /// Arm the two-step delete confirmation for the selected item.
    pub fn begin_delete_selected(&mut self) {
        if self.current_items().is_empty() {
            self.status_message = Some("Žádná položka ke smazání".to_string());
            return;
        }
        self.delete_confirm_active = true;
        self.status_message = Some(
            "Smazat položku trvale z disku? Delete = potvrdit, jiná klávesa = zrušit".to_string(),
        );
    }

    /// Disarm the delete confirmation without deleting anything.
    pub fn cancel_delete(&mut self) {
        self.delete_confirm_active = false;
        self.status_message = None;
    }

    /// Permanently delete the selected item's file from disk and refresh.
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

        let item = proj.items.remove(self.selected_item_idx);
        std::fs::remove_file(&item.file_path)
            .map_err(|e| format!("Smazání selhalo: {}", e))?;

        let title = item.title.clone();
        if self.selected_item_idx >= proj.items.len() {
            self.selected_item_idx = proj.items.len().saturating_sub(1);
        }
        self.viewer_scroll = 0;
        let cwd = self.current_project_path.clone();
        self.refresh(cwd.as_deref(), true);
        self.status_message = Some(format!("Smazáno z disku: {}", title));
        Ok(())
    }
}

#[cfg(test)]
mod tests;
