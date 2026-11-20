//! Test fixture: a record written into the selected project.
use super::discovery::SpaiProjectSummary;
use super::state::SpaiNotesState;
use super::test_emulator::write_note;

impl SpaiNotesState {
    /// Test fixture: writes a record into the selected project and lists it first.
    pub fn create_quick_note_with_status(
        &mut self,
        _title: &str,
        _kind: crate::slices::spai_notes::note::SpaiType,
        _status: crate::slices::spai_notes::note::SpaiStatus,
        body: &str,
    ) -> Result<String, String> {
        let proj: &mut SpaiProjectSummary = self
            .projects
            .get_mut(self.selected_project_idx)
            .ok_or_else(|| "Není vybrán žádný projekt".to_string())?;
        proj.ensure_items();
        let item = write_note(&proj.path, &proj.name, body.trim());
        let id = item.id.clone();
        proj.items.insert(0, item);
        self.selected_item_idx = 0;
        Ok(id)
    }
}
