//! Creation dialog + quick-note behaviour for the SPAI Notes tab (`n` shortcut).
use super::note::{SpaiFacets, SpaiNoteItem, SpaiStatus, SpaiType};
use super::state::SpaiNotesState;
use super::storage_format::{format_spai_markdown, slugify, update_body_status_prefix};
use super::time_utils::current_timestamp_and_date;
use super::type_options::SPAI_TYPE_OPTIONS;

impl SpaiNotesState {
    pub fn open_creation_dialog(&mut self) {
        self.creation_dialog.active = true;
        self.creation_dialog.title_input.clear();
        self.creation_dialog.selected_kind = SpaiType::Todo;
        self.creation_dialog.type_selection = 0;
        self.creation_dialog.autocomplete_active = false;
        self.creation_dialog.autocomplete_selected = 0;
        self.creation_dialog.suggestions.clear();
    }

    pub fn close_creation_dialog(&mut self) {
        self.creation_dialog.active = false;
        self.creation_dialog.title_input.clear();
        self.creation_dialog.type_selection = 0;
        self.creation_dialog.autocomplete_active = false;
        self.creation_dialog.suggestions.clear();
    }

    pub fn on_dialog_char_typed(&mut self, c: char) {
        self.creation_dialog.title_input.push(c);
        self.sync_type_selection_from_input();
        self.update_autocomplete();
    }

    pub fn on_dialog_backspace(&mut self) {
        self.creation_dialog.title_input.pop();
        self.sync_type_selection_from_input();
        self.update_autocomplete();
    }

    pub fn next_type(&mut self) {
        let count = SPAI_TYPE_OPTIONS.len();
        self.creation_dialog.type_selection = (self.creation_dialog.type_selection + 1) % count;
        self.update_selected_kind();
    }

    pub fn prev_type(&mut self) {
        let count = SPAI_TYPE_OPTIONS.len();
        self.creation_dialog.type_selection =
            (self.creation_dialog.type_selection + count - 1) % count;
        self.update_selected_kind();
    }

    fn update_selected_kind(&mut self) {
        if let Some(opt) = SPAI_TYPE_OPTIONS.get(self.creation_dialog.type_selection) {
            self.creation_dialog.selected_kind = opt.kind;
        }
    }

    pub fn apply_selected_type(&mut self) {
        if let Some(opt) = SPAI_TYPE_OPTIONS.get(self.creation_dialog.type_selection) {
            let raw = self.creation_dialog.title_input.trim_start();
            let mut rest = raw;
            for p in &[
                "/. ", "/· ", "/.", "/·", "!- ", "!-", ". ", ".", "/ ", "/", "x ", "X ", "x",
                "X", "z ", "Z ", "z", "Z", "? ", "?", "- ", "-", "+ ", "+", "= ", "=", "* ", "*",
                "% ", "%",
            ] {
                if rest.starts_with(p) {
                    rest = &rest[p.len()..];
                    break;
                }
            }
            let rest = rest.trim_start();
            self.creation_dialog.title_input = format!("{}{}", opt.symbol, rest);
            self.update_autocomplete();
        }
    }

    pub fn sync_type_selection_from_input(&mut self) {
        if let Some(idx) =
            super::type_options::find_type_option_index(&self.creation_dialog.title_input)
        {
            self.creation_dialog.type_selection = idx;
            self.update_selected_kind();
        }
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
        self.next_type();
    }

    pub fn submit_creation_dialog(&mut self) -> Result<String, String> {
        let raw_input = self.creation_dialog.title_input.trim().to_string();
        if raw_input.is_empty() {
            self.apply_selected_type();
            return Err("Název nesmí být prázdný".to_string());
        }

        let detected = super::input_highlighter::detect_spai_input(&raw_input);

        let mut clean_title = raw_input.as_str();
        for p in &[
            "/. ", "/· ", "!- ", ". ", "/ ", "x ", "X ", "z ", "Z ", "? ", "- ", "+ ", "= ", "* ",
            "% ",
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

        let (kind, status, glyph) = if clean_title == raw_input.as_str() {
            if let Some(opt) = SPAI_TYPE_OPTIONS.get(self.creation_dialog.type_selection) {
                (opt.kind, opt.status, opt.symbol)
            } else {
                (detected.kind, detected.status, detected.prefix_glyph)
            }
        } else {
            (detected.kind, detected.status, detected.prefix_glyph)
        };

        let prefix_line = format!("{}{}\n", glyph, title_to_use);
        let res = self.create_quick_note_with_status(&title_to_use, kind, status, &prefix_line);
        if res.is_ok() {
            self.close_creation_dialog();
        }
        res
    }

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

    pub fn create_quick_note(
        &mut self,
        title: &str,
        kind: SpaiType,
        body: &str,
    ) -> Result<String, String> {
        let status = match kind {
            SpaiType::Todo => SpaiStatus::Todo,
            SpaiType::Idea => SpaiStatus::Idea,
            SpaiType::Note => SpaiStatus::Note,
        };
        self.create_quick_note_with_status(title, kind, status, body)
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

#[cfg(test)]
mod tests;

