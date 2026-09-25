//! State management for SPAI Notes tab.
//!
//! Core model + navigation live here. Dialog behaviour is split into sibling
//! impl modules to keep each file small and cohesive:
//! - [`super::state_edit`] — integrated edit dialog (`e`)
//! - [`super::state_creation`] — creation dialog + quick note (`n`)
use super::dialog_state::{NoteCreationDialog, NoteEditDialog};
use super::discovery::{discover_spai_projects, SpaiProjectSummary};
use super::note::SpaiNoteItem;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SpaiNotesState {
    pub projects: Vec<SpaiProjectSummary>,
    pub selected_project_idx: usize,
    pub selected_item_idx: usize,
    pub current_project_path: Option<PathBuf>,
    pub filter_active_project: bool,
    pub viewer_scroll: u16,
    pub edit_mode: bool,
    pub status_message: Option<String>,
    pub creation_dialog: NoteCreationDialog,
    pub edit_dialog: NoteEditDialog,
}

impl Default for SpaiNotesState {
    fn default() -> Self {
        Self::new(None)
    }
}

impl SpaiNotesState {
    pub fn new(current_project_path: Option<PathBuf>) -> Self {
        let projects = discover_spai_projects(current_project_path.as_deref());
        Self {
            projects,
            selected_project_idx: 0,
            selected_item_idx: 0,
            current_project_path,
            filter_active_project: false,
            viewer_scroll: 0,
            edit_mode: false,
            status_message: None,
            creation_dialog: NoteCreationDialog::default(),
            edit_dialog: NoteEditDialog::default(),
        }
    }

    pub fn refresh(&mut self, current_project_path: Option<&Path>) {
        if let Some(cp) = current_project_path {
            self.current_project_path = Some(cp.to_path_buf());
        }
        self.projects = discover_spai_projects(self.current_project_path.as_deref());
        self.clamp_indices();
    }

    fn clamp_indices(&mut self) {
        if self.projects.is_empty() {
            self.selected_project_idx = 0;
            self.selected_item_idx = 0;
            return;
        }

        if self.selected_project_idx >= self.projects.len() {
            self.selected_project_idx = self.projects.len().saturating_sub(1);
        }

        let items_len = self.current_items().len();
        if self.selected_item_idx >= items_len {
            self.selected_item_idx = items_len.saturating_sub(1);
        }
    }

    pub fn current_items(&self) -> &[SpaiNoteItem] {
        if let Some(proj) = self.projects.get(self.selected_project_idx) {
            &proj.items
        } else {
            &[]
        }
    }

    pub fn selected_item(&self) -> Option<&SpaiNoteItem> {
        self.current_items().get(self.selected_item_idx)
    }

    pub fn selected_file_path(&self) -> Option<PathBuf> {
        self.selected_item().map(|it| it.file_path.clone())
    }

    pub fn jump_to_active_project(&mut self) {
        if let Some(curr_path) = &self.current_project_path {
            if let Some(idx) = self.projects.iter().position(|p| p.path == *curr_path) {
                self.selected_project_idx = idx;
                self.selected_item_idx = 0;
                self.viewer_scroll = 0;
                self.status_message = Some(format!("Přepnuto na: {}", self.projects[idx].name));
                return;
            }
        }
        self.status_message = Some("Aktivní projekt nemá docs/spai".to_string());
    }

    pub fn next_project(&mut self) {
        if !self.projects.is_empty() {
            self.selected_project_idx = (self.selected_project_idx + 1) % self.projects.len();
            self.selected_item_idx = 0;
            self.viewer_scroll = 0;
        }
    }

    pub fn prev_project(&mut self) {
        if !self.projects.is_empty() {
            if self.selected_project_idx == 0 {
                self.selected_project_idx = self.projects.len() - 1;
            } else {
                self.selected_project_idx -= 1;
            }
            self.selected_item_idx = 0;
            self.viewer_scroll = 0;
        }
    }

    pub fn next_item(&mut self) {
        let len = self.current_items().len();
        if len > 0 && self.selected_item_idx + 1 < len {
            self.selected_item_idx += 1;
            self.viewer_scroll = 0;
        }
    }

    pub fn prev_item(&mut self) {
        if self.selected_item_idx > 0 {
            self.selected_item_idx -= 1;
            self.viewer_scroll = 0;
        }
    }

    pub fn scroll_viewer_up(&mut self, amount: u16) {
        self.viewer_scroll = self.viewer_scroll.saturating_sub(amount);
    }

    pub fn scroll_viewer_down(&mut self, amount: u16) {
        self.viewer_scroll = self.viewer_scroll.saturating_add(amount);
    }
}
