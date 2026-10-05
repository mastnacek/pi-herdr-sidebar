//! Creation dialog behaviour for the SPAI Notes tab (`n` shortcut).
use super::note::SpaiType;
use super::spai_prefixes::strip_leading_prefix;
use super::state::SpaiNotesState;
use super::text_cursor::{byte_of, char_len};
use super::type_options::SPAI_TYPE_OPTIONS;

impl SpaiNotesState {
    pub fn open_creation_dialog(&mut self) {
        self.creation_dialog.active = true;
        self.creation_dialog.title_input.clear();
        self.creation_dialog.cursor = 0;
        self.creation_dialog.selected_kind = SpaiType::Todo;
        self.creation_dialog.type_selection = 0;
        self.creation_dialog.autocomplete_active = false;
        self.creation_dialog.autocomplete_selected = 0;
        self.creation_dialog.suggestions.clear();
        self.creation_dialog.last_keystroke = None;
        self.creation_dialog.debounced_matches.clear();
        self.creation_dialog.is_evaluating_vector = false;
        self.creation_dialog.vector_evaluated = false;
    }

    pub fn close_creation_dialog(&mut self) {
        self.creation_dialog.active = false;
        self.creation_dialog.title_input.clear();
        self.creation_dialog.cursor = 0;
        self.creation_dialog.type_selection = 0;
        self.creation_dialog.autocomplete_active = false;
        self.creation_dialog.suggestions.clear();
        self.creation_dialog.last_keystroke = None;
        self.creation_dialog.debounced_matches.clear();
        self.creation_dialog.is_evaluating_vector = false;
        self.creation_dialog.vector_evaluated = false;
    }

    pub fn on_dialog_char_typed(&mut self, c: char) {
        let cursor = self.creation_dialog.cursor.min(char_len(&self.creation_dialog.title_input));
        let idx = byte_of(&self.creation_dialog.title_input, cursor);
        self.creation_dialog.title_input.insert(idx, c);
        self.creation_dialog.cursor = cursor + 1;
        self.creation_dialog.last_keystroke = Some(std::time::Instant::now());
        self.creation_dialog.vector_evaluated = false;
        self.creation_dialog.is_evaluating_vector = false;
        self.sync_type_selection_from_input();
        self.update_autocomplete();
    }

    pub fn on_dialog_backspace(&mut self) {
        let cursor = self.creation_dialog.cursor;
        if cursor > 0 {
            let buf = &mut self.creation_dialog.title_input;
            let len = char_len(buf);
            if cursor <= len {
                let start = byte_of(buf, cursor - 1);
                let end = byte_of(buf, cursor);
                buf.replace_range(start..end, "");
                self.creation_dialog.cursor = cursor - 1;
            }
        }
        self.creation_dialog.last_keystroke = Some(std::time::Instant::now());
        self.creation_dialog.vector_evaluated = false;
        self.creation_dialog.is_evaluating_vector = false;
        self.sync_type_selection_from_input();
        self.update_autocomplete();
    }

    pub fn on_dialog_delete(&mut self) {
        let cursor = self.creation_dialog.cursor;
        let buf = &mut self.creation_dialog.title_input;
        if cursor < char_len(buf) {
            let start = byte_of(buf, cursor);
            let end = byte_of(buf, cursor + 1);
            buf.replace_range(start..end, "");
        }
        self.creation_dialog.last_keystroke = Some(std::time::Instant::now());
        self.creation_dialog.vector_evaluated = false;
        self.creation_dialog.is_evaluating_vector = false;
        self.sync_type_selection_from_input();
        self.update_autocomplete();
    }

    pub fn on_dialog_cursor_left(&mut self) {
        self.creation_dialog.cursor = self.creation_dialog.cursor.saturating_sub(1);
    }

    pub fn on_dialog_cursor_right(&mut self) {
        let count = self.creation_dialog.title_input.chars().count();
        self.creation_dialog.cursor = (self.creation_dialog.cursor + 1).min(count);
    }

    pub fn on_dialog_cursor_home(&mut self) {
        self.creation_dialog.cursor = 0;
    }

    pub fn on_dialog_cursor_end(&mut self) {
        self.creation_dialog.cursor = self.creation_dialog.title_input.chars().count();
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
            // Only a full prefix (symbol + space) is stripped, so plain words
            // like "hello" or "xylofon" keep their first letter.
            let rest = strip_leading_prefix(raw).unwrap_or(raw).trim_start();
            self.creation_dialog.title_input = format!("{}{}", opt.symbol, rest);
            self.creation_dialog.cursor = self.creation_dialog.title_input.chars().count();
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
                self.creation_dialog.cursor = self.creation_dialog.title_input.chars().count();
                self.creation_dialog.autocomplete_active = false;
                self.creation_dialog.suggestions.clear();
                return true;
            }
        }
        false
    }

    pub fn submit_creation_dialog(&mut self) -> Result<String, String> {
        let raw_input = self.creation_dialog.title_input.trim().to_string();
        if raw_input.is_empty() {
            self.apply_selected_type();
            return Err("Název nesmí být prázdný".to_string());
        }

        let detected = super::input_highlighter::detect_spai_input(&raw_input);

        // Only a full prefix (symbol + space) is stripped — see spai_prefixes —
        // so "hello" stays "hello".
        let clean_title = strip_leading_prefix(&raw_input)
            .unwrap_or(&raw_input)
            .trim();
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
}

#[cfg(test)]
mod tests;
