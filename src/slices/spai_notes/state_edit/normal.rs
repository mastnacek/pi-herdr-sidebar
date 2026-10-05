//! Normal mode commands of the integrated editor (plan §3): a small, honest
//! set — movement, mode entry, status cycle. No "full Vim".
use super::super::dialog_state::{EditMode, EditField};
use super::super::state::SpaiNotesState;
use super::super::text_cursor::char_len;

impl SpaiNotesState {
    /// `i` — Insert at the cursor; `a` — Insert after the cursor;
    /// `A` — Insert at the end of the current line of the focused field.
    pub fn enter_insert_mode(&mut self, at: At) {
        if at != At::Cursor {
            if let Some(delta) = match at {
                At::AfterCursor => Some(1usize),
                At::LineEnd => None,
                _ => None,
            } {
                self.edit_dialog.cursor = (self.edit_dialog.cursor + delta)
                    .min(match self.edit_dialog.field {
                        EditField::Title => char_len(&self.edit_dialog.title_input),
                        EditField::Body => char_len(&self.edit_dialog.body_input),
                    });
            }
            if at == At::LineEnd {
                let text = match self.edit_dialog.field {
                    EditField::Title => self.edit_dialog.title_input.clone(),
                    EditField::Body => self.edit_dialog.body_input.clone(),
                };
                let (_, end) = super::super::text_cursor::line_bounds(&text, self.edit_dialog.cursor);
                self.edit_dialog.cursor = end;
            }
        }
        self.edit_dialog.mode = EditMode::Insert;
        self.edit_dialog.confirm_discard = false;
    }

    /// `Esc` in Insert — back to Normal.
    pub fn leave_insert_mode(&mut self) {
        self.edit_dialog.mode = EditMode::Normal;
    }

    /// Handles one Normal-mode key. Returns `true` when the key was consumed.
    /// Movement only, plus the mode entries — status cycle and delete flow are
    /// wired in the key dispatch (`x`, `d`/Delete) because they touch disk.
    pub fn on_normal_key(&mut self, code: crossterm::event::KeyCode) -> bool {
        use crossterm::event::KeyCode;
        match code {
            KeyCode::Char('g') => {
                // Top of the body buffer.
                if self.edit_dialog.field == EditField::Body {
                    self.edit_dialog.cursor = 0;
                    self.edit_dialog.body_follow = true;
                }
                true
            }
            KeyCode::Char('G') => {
                if self.edit_dialog.field == EditField::Body {
                    self.edit_dialog.cursor = char_len(&self.edit_dialog.body_input);
                    self.edit_dialog.body_follow = true;
                }
                true
            }
            _ => false,
        }
    }
}

/// Where [`enter_insert_mode`] parks the cursor before switching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum At {
    Cursor,
    AfterCursor,
    LineEnd,
}
