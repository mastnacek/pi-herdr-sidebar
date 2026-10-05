//! Note items lifecycle and status cycling actions.
use super::note::{SpaiFacets, SpaiNoteItem, SpaiStatus, SpaiType};
use super::note_io::{create_note_file, next_spai_number, write_atomic};
use super::state::SpaiNotesState;
use super::storage_format::{format_spai_markdown, slugify, update_body_status_prefix};
use super::time_utils::current_timestamp_and_date;

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
        title: &str,
        kind: SpaiType,
        status: SpaiStatus,
        body: &str,
    ) -> Result<String, String> {
        let proj = self
            .projects
            .get_mut(self.selected_project_idx)
            .ok_or_else(|| "Není vybrán žádný projekt".to_string())?;
        proj.ensure_items();

        std::fs::create_dir_all(&proj.spai_dir)
            .map_err(|e| format!("Složku docs/spai nelze vytvořit: {}", e))?;

        // Next id comes from the disk (max existing number + 1), never from
        // items.len(), so deletions cannot cause id reuse — and pi-spai's own
        // files in the same folder count too.
        let mut next_num = next_spai_number(&proj.spai_dir, &proj.items);
        let slug = slugify(title);
        let (today, timestamp) = current_timestamp_and_date();

        // create_new refuses on collision; bump the id and retry, so an id
        // assigned by pi-spai in between can never cause an overwrite.
        let mut item = None;
        for _ in 0..100 {
            let id = format!("SPAI-{:03}", next_num);
            let file_path = proj
                .spai_dir
                .join(format!("{}-{}-{}.md", today, id, slug));
            let candidate = SpaiNoteItem {
                id,
                title: title.to_string(),
                kind,
                status,
                symbol: status.symbol().to_string(),
                timestamp: timestamp.clone(),
                tags: Vec::new(),
                facets: SpaiFacets {
                    project: Some(proj.name.clone()),
                    project_path: Some(proj.path.to_string_lossy().to_string()),
                    priority: None,
                    deadline: None,
                    ..Default::default()
                },
                body: body.to_string(),
                file_path: file_path.clone(),
                file_name: file_path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("")
                    .to_string(),
                extra_frontmatter: Vec::new(),
            };
            let content = format_spai_markdown(&candidate);
            match create_note_file(&file_path, &content) {
                Ok(()) => {
                    item = Some(candidate);
                    break;
                }
                Err(_) => next_num += 1,
            }
        }
        let item = item.ok_or_else(|| "Nelze najít volné SPAI ID".to_string())?;
        let id = item.id.clone();

        proj.items.insert(0, item);
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
