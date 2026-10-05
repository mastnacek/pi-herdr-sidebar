//! State management for SPAI Notes tab.
//!
//! Core model + navigation live here. Dialog behaviour is split into sibling
//! impl modules to keep each file small and cohesive:
//! - [`super::state_edit`] — integrated edit dialog (`e`)
//! - [`super::state_creation`] — creation dialog + quick note (`n`)
use super::dialog_state::{NoteCreationDialog, NoteEditDialog};
use super::discovery::{
    discover_spai_projects, file_fingerprint, projects_cache_path, SpaiProjectSummary,
};
use super::note::SpaiNoteItem;
use std::path::{Path, PathBuf};

/// Cheap change signal: `(mtime, len)` of pi's project cache and of the selected
/// project's notes dir. Two `stat()` calls stand in for what used to be a full
/// re-read of every note on every refresh tick.
type Fingerprint = (u64, u64, u64, u64);

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
    projects_fingerprint: Fingerprint,
}

impl Default for SpaiNotesState {
    fn default() -> Self {
        Self::new(None)
    }
}

impl SpaiNotesState {
    pub fn new(current_project_path: Option<PathBuf>) -> Self {
        let projects = discover_spai_projects(current_project_path.as_deref());
        let mut state = Self {
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
            projects_fingerprint: (0, 0, 0, 0),
        };
        state.ensure_selected_items();
        state.projects_fingerprint = state.fingerprint();
        state
    }

    pub fn refresh(&mut self, current_project_path: Option<&Path>, force: bool) {
        let cwd_changed =
            current_project_path.is_some_and(|cp| self.current_project_path.as_deref() != Some(cp));
        if let Some(cp) = current_project_path {
            self.current_project_path = Some(cp.to_path_buf());
        }

        let fp = self.fingerprint();
        if !force && !cwd_changed && fp == self.projects_fingerprint {
            return;
        }

        self.projects = discover_spai_projects(self.current_project_path.as_deref());
        self.clamp_indices();
        self.ensure_selected_items();
        self.projects_fingerprint = self.fingerprint();
    }

    fn fingerprint(&self) -> Fingerprint {
        let cache = projects_cache_path()
            .and_then(|p| file_fingerprint(&p))
            .unwrap_or_default();
        let notes = self
            .projects
            .get(self.selected_project_idx)
            .and_then(|p| file_fingerprint(&p.spai_dir))
            .unwrap_or_default();
        (cache.0, cache.1, notes.0, notes.1)
    }

    /// Notes are read on demand — the viewer only ever shows one project.
    fn ensure_selected_items(&mut self) {
        if let Some(proj) = self.projects.get_mut(self.selected_project_idx) {
            proj.ensure_items();
        }
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

    pub fn count_notes_vectors_facets(&mut self) -> (usize, usize, usize) {
        let mut total_notes = 0;
        let mut total_vectors = 0;
        let mut total_classified = 0;
        for p in &mut self.projects {
            p.ensure_items();
            total_notes += p.items.len();
            for item in &p.items {
                if item.facets.area.is_some() && item.facets.effort.is_some() {
                    total_classified += 1;
                }
            }
            if let Some(store) = crate::slices::settings::load_project_vectors(&p.path) {
                total_vectors += store.vectors.len();
            }
        }
        (total_notes, total_vectors, total_classified)
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
                self.ensure_selected_items();
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
            self.ensure_selected_items();
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
            self.ensure_selected_items();
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
