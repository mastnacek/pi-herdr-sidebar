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
}
