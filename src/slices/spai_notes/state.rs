//! State management for SPAI Notes tab.
use super::discovery::{discover_spai_projects, SpaiProjectSummary};
use super::note::{SpaiFacets, SpaiNoteItem, SpaiStatus, SpaiType};
use super::storage_format::{format_spai_markdown, slugify, update_body_status_prefix};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct NoteCreationDialog {
    pub active: bool,
    pub title_input: String,
    pub selected_kind: SpaiType,
    pub autocomplete_active: bool,
    pub autocomplete_selected: usize,
    pub suggestions: Vec<super::autocomplete::ProjectSuggestion>,
}

impl Default for NoteCreationDialog {
    fn default() -> Self {
        Self {
            active: false,
            title_input: String::new(),
            selected_kind: SpaiType::Todo,
            autocomplete_active: false,
            autocomplete_selected: 0,
            suggestions: Vec::new(),
        }
    }
}

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
}

impl Default for SpaiNotesState {
    fn default() -> Self {
        Self::new(None)
    }
}

fn current_timestamp_and_date() -> (String, String) {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    // Fast calendar computation without chrono
    let days = secs / 86400;
    let day_secs = secs % 86400;
    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;

    // Civil day algorithm from Howard Hinnant
    let z = (days as i64) + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    let date_str = format!("{:04}-{:02}-{:02}", y, m, d);
    let time_str = format!("{} {:02}:{:02}:{:02}", date_str, hours, minutes, seconds);

    (date_str, time_str)
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

    /// Jump directly to the current monitored project
    pub fn jump_to_active_project(&mut self) {
        if let Some(curr_path) = &self.current_project_path {
            if let Some(idx) = self.projects.iter().position(|p| p.path == *curr_path) {
                self.selected_project_idx = idx;
                self.selected_item_idx = 0;
                self.viewer_scroll = 0;
                self.status_message = Some(format!(
                    "Přepnuto na aktivní projekt: {}",
                    self.projects[idx].name
                ));
                return;
            }
        }
        self.status_message = Some("Aktivní projekt nemá docs/spai složku".to_string());
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

    /// Creates a quick note in the currently selected project's SPAI dir.
    pub fn open_creation_dialog(&mut self) {
        self.creation_dialog.active = true;
        self.creation_dialog.title_input.clear();
        self.creation_dialog.selected_kind = SpaiType::Todo;
    }

    pub fn close_creation_dialog(&mut self) {
        self.creation_dialog.active = false;
        self.creation_dialog.title_input.clear();
        self.creation_dialog.autocomplete_active = false;
        self.creation_dialog.suggestions.clear();
    }

    pub fn on_dialog_char_typed(&mut self, c: char) {
        self.creation_dialog.title_input.push(c);
        self.update_autocomplete();
    }

    pub fn on_dialog_backspace(&mut self) {
        self.creation_dialog.title_input.pop();
        self.update_autocomplete();
    }

    pub fn update_autocomplete(&mut self) {
        if let Some(query) =
            super::autocomplete::extract_at_query(&self.creation_dialog.title_input)
        {
            let suggestions = super::autocomplete::get_project_suggestions(&self.projects, query);
            self.creation_dialog.autocomplete_active = !suggestions.is_empty();
            self.creation_dialog.suggestions = suggestions;
            self.creation_dialog.autocomplete_selected = 0;
        } else {
            self.creation_dialog.autocomplete_active = false;
            self.creation_dialog.suggestions.clear();
        }
    }

    pub fn next_suggestion(&mut self) {
        if !self.creation_dialog.suggestions.is_empty() {
            self.creation_dialog.autocomplete_selected =
                (self.creation_dialog.autocomplete_selected + 1)
                    % self.creation_dialog.suggestions.len();
        }
    }

    pub fn prev_suggestion(&mut self) {
        if !self.creation_dialog.suggestions.is_empty() {
            if self.creation_dialog.autocomplete_selected == 0 {
                self.creation_dialog.autocomplete_selected =
                    self.creation_dialog.suggestions.len() - 1;
            } else {
                self.creation_dialog.autocomplete_selected -= 1;
            }
        }
    }

    pub fn apply_selected_suggestion(&mut self) -> bool {
        if self.creation_dialog.autocomplete_active && !self.creation_dialog.suggestions.is_empty()
        {
            let idx = self.creation_dialog.autocomplete_selected;
            if let Some(s) = self.creation_dialog.suggestions.get(idx).cloned() {
                super::autocomplete::apply_at_completion(&mut self.creation_dialog.title_input, &s);
                self.creation_dialog.autocomplete_active = false;
                self.creation_dialog.suggestions.clear();
                return true;
            }
        }
        false
    }

    pub fn cycle_creation_kind(&mut self) {
        self.creation_dialog.selected_kind = match self.creation_dialog.selected_kind {
            SpaiType::Todo => SpaiType::Idea,
            SpaiType::Idea => SpaiType::Note,
            SpaiType::Note => SpaiType::Todo,
        };
    }

    pub fn submit_creation_dialog(&mut self) -> Result<String, String> {
        let raw_input = self.creation_dialog.title_input.trim().to_string();
        if raw_input.is_empty() {
            return Err("Název nesmí být prázdný".to_string());
        }

        let detected = super::input_highlighter::detect_spai_input(&raw_input);

        // Strip prefix if user explicitly typed it (e.g. ". my task" -> "my task")
        let mut clean_title = raw_input.as_str();
        for p in &[
            "/. ", "/· ", "!- ", ". ", "/ ", "x ", "X ", "z ", "Z ", "? ", "- ",
        ] {
            if clean_title.starts_with(p) {
                clean_title = &clean_title[p.len()..];
                break;
            }
        }
        let clean_title = clean_title.trim();
        let title_to_use = if clean_title.is_empty() {
            raw_input.clone()
        } else {
            clean_title.to_string()
        };

        let kind = detected.kind;
        let prefix_line = format!("{} {}\n", detected.prefix_glyph, title_to_use);
        let res = self.create_quick_note(&title_to_use, kind, &prefix_line);
        if res.is_ok() {
            self.close_creation_dialog();
        }
        res
    }

    /// Cycles status of the currently selected note (e.g. todo -> working -> waiting -> done -> cancelled)
    pub fn cycle_selected_status(&mut self) -> Result<(), String> {
        let proj = self
            .projects
            .get_mut(self.selected_project_idx)
            .ok_or_else(|| "Není vybrán žádný projekt".to_string())?;

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

    pub fn create_quick_note(
        &mut self,
        title: &str,
        kind: SpaiType,
        body: &str,
    ) -> Result<String, String> {
        let proj = self
            .projects
            .get_mut(self.selected_project_idx)
            .ok_or_else(|| "Není vybrán žádný projekt".to_string())?;

        let next_num = proj.items.len() + 1;
        let id = format!("SPAI-{:03}", next_num);
        let slug = slugify(title);
        let (today, timestamp) = current_timestamp_and_date();
        let file_name = format!("{}-{}-{}.md", today, id, slug);
        let file_path = proj.spai_dir.join(&file_name);

        let status = match kind {
            SpaiType::Todo => SpaiStatus::Todo,
            SpaiType::Idea => SpaiStatus::Idea,
            SpaiType::Note => SpaiStatus::Note,
        };

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
