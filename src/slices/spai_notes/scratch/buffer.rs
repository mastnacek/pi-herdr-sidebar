//! Edit-mode buffer operations for the Scratchpad (plan §1).
//!
//! Char-level editing over the flat line list; Saved lines reject edits so
//! text and file cannot diverge (plan §2.4).
use super::line_model::{LineOrigin, ScratchLine};
use super::state::{byte_of_char, ScratchState};

impl ScratchState {
    /// Inserts a char at the cursor. Rejected on Saved lines.
    pub fn insert_char(&mut self, c: char) -> bool {
        if !self.editable_at_cursor() {
            return false;
        }
        if self.lines.is_empty() {
            self.lines.push(ScratchLine::empty());
        }
        let line = &mut self.lines[self.cursor_line];
        let idx = byte_of_char(&line.text, self.cursor_char);
        line.text.insert(idx, c);
        self.cursor_char += 1;
        self.dirty = true;
        true
    }

    /// Removes the char before the cursor; merges with the previous line at
    /// position 0. Rejected on Saved lines.
    pub fn backspace(&mut self) -> bool {
        if !self.editable_at_cursor() {
            return false;
        }
        if self.lines.is_empty() {
            return false;
        }
        if self.cursor_char > 0 {
            let line = &mut self.lines[self.cursor_line];
            let end = byte_of_char(&line.text, self.cursor_char);
            let start = byte_of_char(&line.text, self.cursor_char - 1);
            line.text.replace_range(start..end, "");
            self.cursor_char -= 1;
            self.dirty = true;
        } else if self.cursor_line > 0 {
            // Merge with the previous line (if it is editable).
            if !self.lines[self.cursor_line - 1].origin.is_editable() {
                return false;
            }
            let prev_len = self.lines[self.cursor_line - 1].text.chars().count();
            let cur = self.lines.remove(self.cursor_line);
            self.lines[self.cursor_line - 1].text.push_str(&cur.text);
            self.cursor_line -= 1;
            self.cursor_char = prev_len;
            self.dirty = true;
        }
        true
    }

    /// Removes the char at the cursor; joins with the next line at EOL.
    pub fn forward_delete(&mut self) -> bool {
        if !self.editable_at_cursor() || self.lines.is_empty() {
            return false;
        }
        let len = self.current_line_len();
        if self.cursor_char < len {
            let line = &mut self.lines[self.cursor_line];
            let start = byte_of_char(&line.text, self.cursor_char);
            let end = byte_of_char(&line.text, self.cursor_char + 1);
            line.text.replace_range(start..end, "");
            self.dirty = true;
        } else if self.cursor_line + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].text.push_str(&next.text);
            self.dirty = true;
        }
        true
    }

    /// Enter: split the current line at the cursor.
    pub fn split_line(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(ScratchLine::empty());
            return;
        }
        let line = &mut self.lines[self.cursor_line];
        let idx = byte_of_char(&line.text, self.cursor_char);
        let tail = line.text.split_off(idx);
        // The tail is a continuation line: it inherits the record origin, so
        // a Saved record's prose stays marked as part of the file-backed group.
        let new_origin = match &line.origin {
            LineOrigin::New => LineOrigin::New,
            o => o.clone(),
        };
        self.lines.insert(
            self.cursor_line + 1,
            ScratchLine {
                text: tail,
                origin: new_origin,
            },
        );
        self.cursor_line += 1;
        self.cursor_char = 0;
        self.dirty = true;
    }

    pub fn cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_char = self.cursor_char.min(self.current_line_len());
        }
    }

    pub fn cursor_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_char = self.cursor_char.min(self.current_line_len());
        }
    }
}

impl ScratchLine {
    pub fn empty() -> Self {
        Self {
            text: String::new(),
            origin: LineOrigin::New,
        }
    }
}