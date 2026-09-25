//! Integrated edit dialog behaviour for the SPAI Notes tab (`e` shortcut).
//!
//! Editing model: the dialog holds both fields and a single cursor expressed as
//! a **char** index into the focused field, so diacritics and emoji edit
//! correctly. Pure position arithmetic lives in [`super::text_cursor`], the
//! body's row wrapping in [`super::text_layout`].
use super::dialog_state::EditField;
use super::state::SpaiNotesState;
use super::storage_format::format_spai_markdown;
use super::text_cursor::{byte_of, char_len, line_bounds, move_line};

impl SpaiNotesState {
    /// Opens the integrated editor pre-filled from the selected note.
    pub fn open_edit_dialog(&mut self) {
        let (title, body) = if let Some(item) = self.selected_item() {
            (item.title.clone(), item.body.clone())
        } else {
            return;
        };
        self.edit_dialog.active = true;
        self.edit_dialog.title_input = title;
        self.edit_dialog.body_input = body;
        self.edit_dialog.field = EditField::Title;
        self.edit_dialog.cursor = char_len(&self.edit_dialog.title_input);
        self.edit_dialog.body_scroll = 0;
        self.edit_dialog.body_follow = true;
    }

    pub fn close_edit_dialog(&mut self) {
        self.edit_dialog.active = false;
        self.edit_dialog.title_input.clear();
        self.edit_dialog.body_input.clear();
        self.edit_dialog.field = EditField::Title;
        self.edit_dialog.cursor = 0;
        self.edit_dialog.body_scroll = 0;
        self.edit_dialog.body_follow = true;
    }

    /// Switches Title ↔ Body and parks the cursor at the end of the new field.
    pub fn toggle_edit_field(&mut self) {
        self.edit_dialog.field = match self.edit_dialog.field {
            EditField::Title => EditField::Body,
            EditField::Body => EditField::Title,
        };
        self.edit_dialog.cursor = match self.edit_dialog.field {
            EditField::Title => char_len(&self.edit_dialog.title_input),
            EditField::Body => char_len(&self.edit_dialog.body_input),
        };
        self.edit_dialog.body_follow = true;
    }

    fn focused_text_mut(&mut self) -> &mut String {
        match self.edit_dialog.field {
            EditField::Title => &mut self.edit_dialog.title_input,
            EditField::Body => &mut self.edit_dialog.body_input,
        }
    }

    pub fn on_edit_char_typed(&mut self, c: char) {
        let cursor = self.edit_dialog.cursor;
        let buf = self.focused_text_mut();
        buf.insert(byte_of(buf, cursor), c);
        self.edit_dialog.cursor = cursor + 1;
        self.edit_dialog.body_follow = true;
    }

    pub fn on_edit_backspace(&mut self) {
        let cursor = self.edit_dialog.cursor;
        if cursor == 0 {
            return;
        }
        let buf = self.focused_text_mut();
        let start = byte_of(buf, cursor - 1);
        let end = byte_of(buf, cursor);
        buf.replace_range(start..end, "");
        self.edit_dialog.cursor = cursor - 1;
        self.edit_dialog.body_follow = true;
    }

    /// Forward delete (Del) at the cursor.
    pub fn on_edit_delete(&mut self) {
        let cursor = self.edit_dialog.cursor;
        let buf = self.focused_text_mut();
        if cursor >= char_len(buf) {
            return;
        }
        let start = byte_of(buf, cursor);
        let end = byte_of(buf, cursor + 1);
        buf.replace_range(start..end, "");
        self.edit_dialog.body_follow = true;
    }

    pub fn on_edit_cursor_left(&mut self) {
        self.edit_dialog.cursor = self.edit_dialog.cursor.saturating_sub(1);
        self.edit_dialog.body_follow = true;
    }

    pub fn on_edit_cursor_right(&mut self) {
        let max = match self.edit_dialog.field {
            EditField::Title => char_len(&self.edit_dialog.title_input),
            EditField::Body => char_len(&self.edit_dialog.body_input),
        };
        self.edit_dialog.cursor = (self.edit_dialog.cursor + 1).min(max);
        self.edit_dialog.body_follow = true;
    }

    pub fn on_edit_cursor_line_start(&mut self) {
        let cursor = self.edit_dialog.cursor;
        let text = match self.edit_dialog.field {
            EditField::Title => &self.edit_dialog.title_input,
            EditField::Body => &self.edit_dialog.body_input,
        };
        let (start, _) = line_bounds(text, cursor);
        self.edit_dialog.cursor = start;
        self.edit_dialog.body_follow = true;
    }

    pub fn on_edit_cursor_line_end(&mut self) {
        let cursor = self.edit_dialog.cursor;
        let text = match self.edit_dialog.field {
            EditField::Title => &self.edit_dialog.title_input,
            EditField::Body => &self.edit_dialog.body_input,
        };
        let (_, end) = line_bounds(text, cursor);
        self.edit_dialog.cursor = end;
        self.edit_dialog.body_follow = true;
    }

    /// Moves the cursor one line up/down inside the body (no-op for the title).
    pub fn on_edit_cursor_vertical(&mut self, down: bool) {
        if self.edit_dialog.field != EditField::Body {
            return;
        }
        let cursor = self.edit_dialog.cursor;
        self.edit_dialog.cursor = move_line(&self.edit_dialog.body_input, cursor, down);
        self.edit_dialog.body_follow = true;
    }

    /// Enter in the title field advances to the body; Enter in the body inserts a newline.
    pub fn on_edit_enter(&mut self) {
        match self.edit_dialog.field {
            EditField::Title => self.toggle_edit_field(),
            EditField::Body => self.on_edit_char_typed('\n'),
        }
    }

    /// Manual body scroll (PgUp/PgDn) — pauses cursor auto-follow.
    pub fn scroll_edit_body(&mut self, down: bool, amount: u16) {
        self.edit_dialog.body_follow = false;
        self.edit_dialog.body_scroll = if down {
            self.edit_dialog.body_scroll.saturating_add(amount)
        } else {
            self.edit_dialog.body_scroll.saturating_sub(amount)
        };
    }

    /// Writes title + body back to the note markdown file.
    pub fn submit_edit_dialog(&mut self) -> Result<(), String> {
        let new_title = self.edit_dialog.title_input.trim().to_string();
        if new_title.is_empty() {
            return Err("Název nesmí být prázdný".to_string());
        }
        let new_body = self.edit_dialog.body_input.clone();

        let proj = self
            .projects
            .get_mut(self.selected_project_idx)
            .ok_or_else(|| "Není vybrán žádný projekt".to_string())?;

        let item = proj
            .items
            .get_mut(self.selected_item_idx)
            .ok_or_else(|| "Není vybrána žádná položka".to_string())?;

        item.title = new_title;
        item.body = new_body;
        let updated_content = format_spai_markdown(item);
        std::fs::write(&item.file_path, updated_content).map_err(|e| e.to_string())?;
        self.status_message = Some("Poznámka uložena".to_string());
        self.close_edit_dialog();

        Ok(())
    }
}

#[cfg(test)]
mod tests;
