//! Note items lifecycle and status cycling actions.
use super::note::{SpaiFacets, SpaiNoteItem, SpaiStatus, SpaiType};
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
            .get_mut(self.selected_item_idx)
            .ok_or_else(|| "Není vybrána žádná položka".to_string())?;

        let new_status = item.status.next_cycle();
        item.status = new_status;
        item.symbol = new_status.symbol().to_string();
        item.body = update_body_status_prefix(&item.body, new_status);

        let updated_content = format_spai_markdown(item);
        std::fs::write(&item.file_path, updated_content).map_err(|e| e.to_string())?;
        self.status_message = Some(format!(
            "Stav změněn: {} {}",
            item.status.glyph(),
            item.status.as_str()
        ));

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

        let next_num = proj.items.len() + 1;
        let id = format!("SPAI-{:03}", next_num);
        let slug = slugify(title);
        let (today, timestamp) = current_timestamp_and_date();
        let file_name = format!("{}-{}-{}.md", today, id, slug);
        let file_path = proj.spai_dir.join(&file_name);

        let item = SpaiNoteItem {
            id: id.clone(),
            title: title.to_string(),
            kind,
            status,
            symbol: status.symbol().to_string(),
            timestamp,
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
            file_name,
        };

        let content = format_spai_markdown(&item);
        std::fs::write(&file_path, content).map_err(|e| e.to_string())?;

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
mod tests {
    use super::*;
    use crate::slices::spai_notes::discovery::SpaiProjectSummary;
    use std::fs;

    fn temp_project() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("spai_del_test_{}_{}", std::process::id(), nanos));
        fs::create_dir_all(dir.join("docs").join("spai")).unwrap();
        dir
    }

    fn write_note(project: &std::path::Path, id: u32, title: &str) -> std::path::PathBuf {
        let path = project
            .join("docs")
            .join("spai")
            .join(format!("2026-01-01-SPAI-{id:03}-{title}.md"));
        fs::write(
            &path,
            format!(
                "---\ntype: Todo\ntitle: \"{title}\"\ntimestamp: 1\nstatus: todo\nsource: pi-spai\nspai_symbol: '.'\n---\n\n# SPAI-{id:03}: {title}\n. {title}\n"
            ),
        )
        .unwrap();
        path
    }

    fn state_on(project: &std::path::Path) -> SpaiNotesState {
        let mut state = SpaiNotesState::new(Some(project.to_path_buf()));
        let idx = state
            .projects
            .iter()
            .position(|p| p.path == project)
            .expect("temp project discovered");
        state.selected_project_idx = idx;
        state
    }

    #[test]
    fn delete_removes_file_from_disk_and_drops_the_item() {
        let project = temp_project();
        let keep = write_note(&project, 1, "keep");
        let gone = write_note(&project, 2, "gone");
        let mut state = state_on(&project);
        assert_eq!(state.current_items().len(), 2);

        state.selected_item_idx = 1;
        state.begin_delete_selected();
        assert!(state.delete_confirm_active, "first Delete arms the confirm");

        state.confirm_delete_selected().unwrap();

        assert!(!gone.exists(), "selected note file must be gone from disk");
        assert!(keep.exists(), "other note files must survive");
        assert!(!state.delete_confirm_active);
        let titles: Vec<&str> = state
            .current_items()
            .iter()
            .map(|i| i.title.as_str())
            .collect();
        assert!(!titles.contains(&"gone"), "item removed from state: {titles:?}");

        fs::remove_dir_all(&project).ok();
    }

    #[test]
    fn cancelled_delete_keeps_everything() {
        let project = temp_project();
        let note = write_note(&project, 1, "safe");
        let mut state = state_on(&project);

        state.begin_delete_selected();
        assert!(state.delete_confirm_active);
        state.cancel_delete();

        assert!(note.exists(), "cancel must keep the file on disk");
        assert_eq!(state.current_items().len(), 1);
        assert!(!state.delete_confirm_active);

        fs::remove_dir_all(&project).ok();
    }

    #[test]
    fn delete_on_empty_list_is_a_no_op_message() {
        let project = temp_project();
        let mut state = state_on(&project);
        assert!(state.current_items().is_empty());

        state.begin_delete_selected();
        assert!(!state.delete_confirm_active, "nothing to delete, no arming");
        assert!(state.status_message.is_some());

        fs::remove_dir_all(&project).ok();
    }
}
