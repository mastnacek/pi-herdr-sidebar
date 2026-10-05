//! Integrated edit dialog behaviour for the SPAI Notes tab (`e` shortcut).
//!
//! Vim-like modes (plan §3, 2026-10-05 decision): the dialog opens in
//! **Normal** (read), `i`/`a`/`A` switch to **Insert** (today's editing
//! behaviour), `Esc` returns to Normal, `Esc`/`q` in Normal closes (dirty →
//! confirm discard). The fields stay separate: `Tab` switches Title ↔ Body,
//! saving writes the same `format_spai_markdown`.
//!
//! Split for the 300-line cap:
//! - [`super::state_edit::insert`] — Insert mode (text entry, today's model)
//! - [`super::state_edit::normal`] — Normal mode commands
mod insert;
mod normal;

pub use normal::At;

use super::dialog_state::{EditMode, EditField};
use super::note_io::write_atomic;
use super::state::SpaiNotesState;
use super::storage_format::format_spai_markdown;
use super::text_cursor::char_len;

impl SpaiNotesState {
    /// Opens the integrated editor pre-filled from the selected note, in Normal.
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
        self.edit_dialog.mode = EditMode::Normal;
        self.edit_dialog.dirty = false;
        self.edit_dialog.confirm_discard = false;
        self.edit_dialog.cursor = char_len(&self.edit_dialog.title_input);
        self.edit_dialog.body_scroll = 0;
        self.edit_dialog.body_follow = true;
    }

    pub fn close_edit_dialog(&mut self) {
        self.edit_dialog.active = false;
        self.edit_dialog.title_input.clear();
        self.edit_dialog.body_input.clear();
        self.edit_dialog.field = EditField::Title;
        self.edit_dialog.mode = EditMode::Normal;
        self.edit_dialog.dirty = false;
        self.edit_dialog.confirm_discard = false;
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

    /// `Esc`/`q` in Normal: close, or arm the discard confirmation when dirty.
    /// Returns `true` when the dialog actually closed.
    pub fn request_close_edit_dialog(&mut self) -> bool {
        if self.edit_dialog.dirty && !self.edit_dialog.confirm_discard {
            self.edit_dialog.confirm_discard = true;
            self.status_message =
                Some("Neuložené změny — Esc znovu zahodí, Ctrl+S uloží".to_string());
            return false;
        }
        self.close_edit_dialog();
        true
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
            .get(self.selected_item_idx)
            .ok_or_else(|| "Není vybrána žádná položka".to_string())?;

        // Write first (crash-safe, atomically), then update the in-memory
        // state, so a failed write never leaves the UI describing a disk
        // state that does not exist.
        let mut updated = item.clone();
        updated.title = new_title;
        updated.body = new_body;
        let updated_content = format_spai_markdown(&updated);
        write_atomic(&updated.file_path, &updated_content)?;

        let item = proj
            .items
            .get_mut(self.selected_item_idx)
            .ok_or_else(|| "Není vybrána žádná položka".to_string())?;
        item.title = updated.title;
        item.body = updated.body;
        self.status_message = Some("Poznámka uložena".to_string());
        self.close_edit_dialog();

        Ok(())
    }
}
