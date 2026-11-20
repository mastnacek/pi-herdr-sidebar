//! State management for SPAI Notes tab.
//!
//! Core model + navigation live here. Dialog behaviour is split into sibling
//! impl modules to keep each file small and cohesive:
//! - [`super::state_edit`] — integrated edit dialog (`e`)
//! - [`super::state_creation`] — creation dialog + quick note (`n`)
use super::dialog_state::{DedupPanel, NoteCreationDialog, NoteEditDialog};
use super::discovery::{
    discover_spai_projects, file_fingerprint, projects_cache_path, SpaiProjectSummary,
};
use super::note::SpaiNoteItem;
use super::uma_links::UmaMemoryLink;
use std::path::{Path, PathBuf};

/// Cheap change signal: `(mtime, len)` pairs from a few `stat()` calls stand in for
/// a full re-read of every note on every refresh tick. The project list and the
/// selected project's notes have separate ones, so a note change never rebuilds
/// the list (and a list change never reloads notes).
pub(super) type ListFingerprint = (u64, u64);
pub(super) type NotesFingerprint = (u64, u64, u64, u64);

#[derive(Debug)]
pub struct SpaiNotesState {
    pub projects: Vec<SpaiProjectSummary>,
    pub selected_project_idx: usize,
    pub selected_item_idx: usize,
    pub current_project_path: Option<PathBuf>,
    #[allow(dead_code)]
    pub filter_active_project: bool,
    pub viewer_scroll: u16,
    #[allow(dead_code)]
    pub edit_mode: bool,
    pub status_message: Option<String>,
    /// Two-step delete guard: first Delete arms, second Delete removes from disk.
    pub delete_confirm_active: bool,
    pub creation_dialog: NoteCreationDialog,
    pub edit_dialog: NoteEditDialog,
    /// Scratchpad modal (docs/scratchpad_mode_plan.md).
    pub scratch: super::scratch::state::ScratchState,
    /// On-demand duplicate-check panel (Ctrl+D); polls a finished background
    /// job only — never runs on its own.
    pub dedup: DedupPanel,
    /// How the sidebar reaches `spai` (a process; tests use a scripted one).
    pub core: crate::shared::spai_client::CoreConfig,
    /// A creation-dialog `spai add` running in the background.
    pub add_job: Option<super::state_add::AddJob>,
    /// Records with a vector (`spai projects`), refreshed in the background.
    pub vector_total: usize,
    pub vectors_stale: bool,
    pub(super) vector_rx: Option<std::sync::mpsc::Receiver<Result<usize, String>>>,
    /// Fingerprint of pi's project cache (the fallback list's source).
    pub(super) projects_fingerprint: ListFingerprint,
    /// Selected project + fingerprint of its notes and .uma dirs.
    pub(super) notes_fingerprint: (Option<PathBuf>, NotesFingerprint),
    /// Set once `spai projects` has answered: the fallback list is never used again.
    pub(super) spai_listed: bool,
    /// Type-to-search line over the project list (`/`).
    pub project_filter: Option<super::state_project_filter::ProjectFilter>,
    /// `spai projects` running in the background (see `state_projects`).
    pub(super) projects_rx: Option<
        std::sync::mpsc::Receiver<Result<Vec<crate::shared::spai_client::ListedProject>, String>>,
    >,
}

/// Tests never start the real `spai` (it would touch real backlogs): they get the emulator.
fn default_core() -> crate::shared::spai_client::CoreConfig {
    #[cfg(test)]
    {
        super::test_emulator::config()
    }
    #[cfg(not(test))]
    {
        crate::shared::spai_client::CoreConfig::from_process()
    }
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
            delete_confirm_active: false,
            creation_dialog: NoteCreationDialog::default(),
            edit_dialog: NoteEditDialog::default(),
            scratch: super::scratch::state::ScratchState::default(),
            dedup: DedupPanel::default(),
            core: default_core(),
            add_job: None,
            vector_total: 0,
            vectors_stale: true,
            vector_rx: None,
            projects_fingerprint: (0, 0),
            notes_fingerprint: (None, (0, 0, 0, 0)),
            spai_listed: false,
            project_filter: None,
            projects_rx: None,
        };
        state.ensure_selected_items();
        state.projects_fingerprint = state.list_fingerprint();
        state.notes_fingerprint = state.notes_fingerprint_now();
        state.request_project_list();
        state
    }

    pub fn refresh(&mut self, current_project_path: Option<&Path>, force: bool) {
        let cwd_changed =
            current_project_path.is_some_and(|cp| self.current_project_path.as_deref() != Some(cp));
        if let Some(cp) = current_project_path {
            self.current_project_path = Some(cp.to_path_buf());
        }

        // The project list: only the cache, the cwd or `force` ask for it again.
        let list_fp = self.list_fingerprint();
        if force || cwd_changed || list_fp != self.projects_fingerprint {
            self.projects_fingerprint = list_fp;
            if !self.spai_listed {
                let selected = self.selected_project_path();
                self.projects = discover_spai_projects(self.current_project_path.as_deref());
                self.reselect_project(selected);
            }
            self.request_project_list();
        }

        // The notes: only the selected project's own folders reload its items.
        let (path, fp) = self.notes_fingerprint_now();
        let same_project = self.notes_fingerprint.0 == path;
        if force || (same_project && fp != self.notes_fingerprint.1) {
            if let Some(proj) = self.projects.get_mut(self.selected_project_idx) {
                proj.reload_items();
            }
            self.clamp_indices();
        }
        self.ensure_selected_items();
        self.notes_fingerprint = self.notes_fingerprint_now();
    }

    pub(super) fn list_fingerprint(&self) -> ListFingerprint {
        projects_cache_path()
            .and_then(|p| file_fingerprint(&p))
            .unwrap_or_default()
    }

    pub(super) fn notes_fingerprint_now(&self) -> (Option<PathBuf>, NotesFingerprint) {
        let selected = self.projects.get(self.selected_project_idx);
        let notes = selected
            .and_then(|p| file_fingerprint(&p.spai_dir))
            .unwrap_or_default();
        let uma = selected
            .and_then(|p| file_fingerprint(&p.uma_dir))
            .unwrap_or_default();
        (
            selected.map(|p| p.path.clone()),
            (notes.0, notes.1, uma.0, uma.1),
        )
    }

    pub(super) fn selected_project_path(&self) -> Option<PathBuf> {
        self.projects
            .get(self.selected_project_idx)
            .map(|p| p.path.clone())
    }

    /// After the list was rebuilt: keeps the selection by path, never by index.
    pub(super) fn reselect_project(&mut self, selected: Option<PathBuf>) {
        match selected.and_then(|s| self.projects.iter().position(|p| p.path == s)) {
            Some(at) => self.selected_project_idx = at,
            None => {
                self.selected_project_idx = 0;
                self.selected_item_idx = 0;
            }
        }
        self.clamp_indices();
        self.ensure_selected_items();
    }

    /// Notes are read on demand — the viewer only ever shows one project.
    pub(super) fn ensure_selected_items(&mut self) {
        if let Some(proj) = self.projects.get_mut(self.selected_project_idx) {
            proj.ensure_items();
        }
    }

    pub(super) fn clamp_indices(&mut self) {
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
        let mut total_classified = 0;
        for p in &mut self.projects {
            p.ensure_items();
            total_notes += p.items.len();
            for item in &p.items {
                if item.facets.area.is_some() && item.facets.effort.is_some() {
                    total_classified += 1;
                }
            }
        }
        self.poll_vector_total();
        (total_notes, self.vector_total, total_classified)
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

    pub fn selected_memory_links(&self) -> &[UmaMemoryLink] {
        let Some(item) = self.selected_item() else {
            return &[];
        };
        self.projects
            .get(self.selected_project_idx)
            .map(|project| project.memory_links_for(item))
            .unwrap_or(&[])
    }

    pub fn selected_file_path(&self) -> Option<PathBuf> {
        self.selected_item().map(|it| it.file_path.clone())
    }

    pub fn jump_to_active_project(&mut self) {
        if let Some(curr_path) = &self.current_project_path {
            if let Some(idx) = self.projects.iter().position(|p| p.path == *curr_path) {
                self.select_project(idx);
                self.status_message = Some(format!("Přepnuto na: {}", self.projects[idx].name));
                return;
            }
        }
        self.status_message = Some("Aktivní projekt nemá docs/spai".to_string());
    }

    pub fn next_project(&mut self) {
        self.step_project(true);
    }

    pub fn prev_project(&mut self) {
        self.step_project(false);
    }

    /// Selects project `idx` and resets the note cursor.
    pub(super) fn select_project(&mut self, idx: usize) {
        self.selected_project_idx = idx;
        self.selected_item_idx = 0;
        self.viewer_scroll = 0;
        self.ensure_selected_items();
        self.notes_fingerprint = self.notes_fingerprint_now();
    }

    /// One project forward/back, wrapping; while the filter is open only matches count.
    fn step_project(&mut self, forward: bool) {
        let ring: Vec<usize> = match &self.project_filter {
            Some(_) => self.project_filter_matches(),
            None => (0..self.projects.len()).collect(),
        };
        if ring.is_empty() {
            return;
        }
        let at = ring.iter().position(|&i| i == self.selected_project_idx);
        let next = match (at, forward) {
            (Some(a), true) => ring[(a + 1) % ring.len()],
            (Some(a), false) => ring[(a + ring.len() - 1) % ring.len()],
            (None, true) => ring[0],
            (None, false) => ring[ring.len() - 1],
        };
        self.select_project(next);
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
