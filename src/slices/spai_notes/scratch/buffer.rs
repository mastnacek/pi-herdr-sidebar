//! Edit-mode buffer operations for the Scratchpad (plan §1).
//!
//! Char-level editing over the flat line list; Saved lines reject edits so
//! text and file cannot diverge (plan §2.4).
use super::line_model::{LineOrigin, ScratchLine};
use super::state::{byte_of_char, ScratchState};
use crate::slices::spai_notes::spai_prefixes::SPAI_PREFIXES;
use crate::slices::spai_notes::time_utils::current_stamp_czech;

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

        if c == ' ' && self.just_completed_bare_prefix() {
            // The entry type was just recognised (piprompt-core rule: exactly
            // a bare prefix + its space at line start) — stamp the moment.
            let stamp = current_stamp_czech();
            let stamp_chars = stamp.chars().count();
            let line = &mut self.lines[self.cursor_line];
            let idx = byte_of_char(&line.text, self.cursor_char);
            line.text.insert_str(idx, &format!("{} ", stamp));
            self.cursor_char += stamp_chars + 1;
        }
        true
    }

    /// Did the caret just complete a bare prefix? Reads the current line from
    /// its start up to the cursor — a prefix typed after prose stays prose
    /// (same rule as `piprompt_core::spai::is_bare_prefix`).
    fn just_completed_bare_prefix(&self) -> bool {
        let Some(line) = self.current_line() else {
            return false;
        };
        let chars: Vec<char> = line.text.chars().collect();
        let upto = self.cursor_char.min(chars.len());
        let before: String = chars[..upto].iter().collect();
        SPAI_PREFIXES.iter().any(|p| *p == before)
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

    /// Replaces the `@token` before the cursor with `@name` and moves past it.
    /// Returns `false` when the cursor is not inside a mention.
    pub fn complete_mention(&mut self, name: &str) -> bool {
        let Some((at, _filter)) = self.mention_context() else {
            return false;
        };
        let line = &mut self.lines[self.cursor_line];
        let start = byte_of_char(&line.text, at);
        let end = byte_of_char(&line.text, self.cursor_char);
        let completed = format!("@{} ", name);
        line.text.replace_range(start..end, &completed);
        self.cursor_char = at + completed.chars().count();
        self.dirty = true;
        true
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::slices::spai_notes::time_utils::current_stamp_czech;

    fn state() -> ScratchState {
        let mut s = ScratchState::default();
        s.lines.push(ScratchLine::empty());
        s
    }

    #[test]
    fn a_bare_prefix_completed_with_space_inserts_the_czech_stamp() {
        let mut s = state();
        for c in ". ".chars() {
            s.insert_char(c);
        }
        let stamp = current_stamp_czech();
        assert_eq!(s.lines[0].text, format!(". {} ", stamp));
        assert_eq!(s.cursor_char, 2 + stamp.chars().count() + 1);
    }

    #[test]
    fn a_prefix_after_prose_is_not_stamped() {
        let mut s = state();
        for c in "viděl jsem ".chars() {
            s.insert_char(c);
        }
        for c in ". ".chars() {
            s.insert_char(c);
        }
        assert_eq!(s.lines[0].text, "viděl jsem . ");
    }

    #[test]
    fn typing_on_after_the_stamp_does_not_stamp_again() {
        let mut s = state();
        for c in ". ".chars() {
            s.insert_char(c);
        }
        let stamped = s.lines[0].text.clone();
        for c in "opravit ".chars() {
            s.insert_char(c);
        }
        assert_eq!(s.lines[0].text, format!("{}opravit ", stamped));
    }

    #[test]
    fn multi_char_prefixes_stamp_too() {
        let mut s = state();
        for c in "/. ".chars() {
            s.insert_char(c);
        }
        assert!(s.lines[0].text.starts_with("/. ["));
    }

    #[test]
    fn stamps_are_one_edit_and_backspace_removes_them_normally() {
        let mut s = state();
        for c in ". ".chars() {
            s.insert_char(c);
        }
        assert!(s.dirty);
        // Backspace walks back through the stamp like ordinary text.
        for _ in 0..(current_stamp_czech().chars().count() + 1) {
            s.backspace();
        }
        assert_eq!(s.lines[0].text, ". ");
    }
}
