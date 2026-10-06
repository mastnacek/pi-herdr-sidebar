//! `@` project autocomplete for the Scratchpad — **the same engine as the
//! smart input** (`n` in the Notes tab): [`crate::slices::spai_notes::
//! autocomplete`] supplies matching, `insert_text` (`@name`, `@"name with
//! spaces"`) and the wrap-around navigation, so the two surfaces cannot
//! disagree. The popup opens the moment a mention token (`@` + anything up
//! to the next whitespace) is open under the caret.
use super::state::{byte_of_char, MentionPopup, ScratchState};
use crate::slices::spai_notes::autocomplete::{get_project_suggestions, ProjectSuggestion};
use crate::slices::spai_notes::discovery::SpaiProjectSummary;

/// Suggestions for the `@token` currently being typed in `state`, from the
/// same filter the smart input uses (case-insensitive contains on name or
/// path, quoted `insert_text` for names with spaces).
pub fn mention_matches(
    state: &ScratchState,
    projects: &[SpaiProjectSummary],
) -> Vec<ProjectSuggestion> {
    let filter = state
        .mention_context()
        .map(|(_, f)| f)
        .unwrap_or_default();
    get_project_suggestions(projects, &filter)
}

impl ScratchState {
    /// The `@token` being typed before the cursor on the current line:
    /// `Some((char index of '@', filter characters))`, or `None`.
    ///
    /// The scan stops at the first whitespace before the cursor, so
    /// `mail@example.com` mid-line does not open the popup unless the caret
    /// sits inside that token (Esc dismisses it if it does). Equivalent to
    /// `autocomplete::extract_at_query` for the caret-at-end case, but
    /// cursor-aware for the middle of a scratchpad line.
    pub fn mention_context(&self) -> Option<(usize, String)> {
        let line = self.current_line()?;
        if !line.origin.is_editable() {
            return None;
        }
        let chars: Vec<char> = line.text.chars().collect();
        let upto = self.cursor_char.min(chars.len());
        let mut at = None;
        for i in (0..upto).rev() {
            if chars[i] == '@' {
                at = Some(i);
                break;
            }
            if chars[i].is_whitespace() {
                return None;
            }
        }
        let at = at?;
        let filter: String = chars[at + 1..upto].iter().collect();
        Some((at, filter))
    }

    /// Recomputes the popup after any text change: opens when a mention token
    /// is open under the caret, closes otherwise. The selection survives a
    /// re-filter on the same line; a new line starts at 0.
    pub fn update_mention_popup(&mut self) {
        self.mention = match self.mention_context() {
            Some(_) => {
                let selected = match &self.mention {
                    Some(m) if m.anchor_line == self.cursor_line => m.selected,
                    _ => 0,
                };
                Some(MentionPopup {
                    anchor_line: self.cursor_line,
                    selected,
                })
            }
            None => None,
        };
    }

    /// Moves the popup selection with the smart input's wrap-around
    /// (`next_suggestion`/`prev_suggestion` semantics).
    pub fn mention_move_selection(&mut self, projects: &[SpaiProjectSummary], delta: i32) {
        let len = mention_matches(self, projects).len();
        if len == 0 {
            return;
        }
        let Some(popup) = &mut self.mention else {
            return;
        };
        let len_i = len as i32;
        let wrapped = ((popup.selected as i32 + delta) % len_i + len_i) % len_i;
        popup.selected = wrapped as usize;
    }

    /// Accepts the selected suggestion exactly like the smart input's
    /// `apply_selected_suggestion`: the `@filter` token is replaced with
    /// `insert_text` (`@name`, `@"name with spaces"`) plus a trailing space,
    /// the caret lands after it, and the popup closes.
    pub fn accept_mention(&mut self, projects: &[SpaiProjectSummary]) -> bool {
        let Some(popup) = &self.mention else {
            return false;
        };
        let selected = popup.selected;
        let matches = mention_matches(self, projects);
        let Some(s) = matches.get(selected) else {
            return false;
        };
        let Some((at, _filter)) = self.mention_context() else {
            return false;
        };

        let completed = format!("{} ", s.insert_text);
        let line = &mut self.lines[self.cursor_line];
        let start = byte_of_char(&line.text, at);
        let end = byte_of_char(&line.text, self.cursor_char);
        line.text.replace_range(start..end, &completed);
        self.cursor_char = at + completed.chars().count();
        self.dirty = true;
        self.mention = None;
        true
    }

    /// Closes the `@` popup (Esc), keeping the typed token as-is.
    pub fn close_mention_popup(&mut self) {
        self.mention = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slices::spai_notes::scratch::line_model::{LineOrigin, ScratchLine};
    use std::path::PathBuf;

    fn projects() -> Vec<SpaiProjectSummary> {
        vec![
            SpaiProjectSummary::new(
                "herdr".to_string(),
                PathBuf::from("D:/work/herdr"),
                PathBuf::from("D:/work/herdr/docs/spai"),
            ),
            SpaiProjectSummary::new(
                "pi-spai".to_string(),
                PathBuf::from("D:/work/pi-spai"),
                PathBuf::from("D:/work/pi-spai/docs/spai"),
            ),
            SpaiProjectSummary::new(
                "mojek projekty".to_string(),
                PathBuf::from("D:/work/mojek"),
                PathBuf::from("D:/work/mojek/docs/spai"),
            ),
        ]
    }

    fn state_with(line: &str, cursor_char: usize) -> ScratchState {
        let mut s = ScratchState::default();
        s.lines = vec![ScratchLine {
            text: line.to_string(),
            origin: LineOrigin::New,
        }];
        s.cursor_line = 0;
        s.cursor_char = cursor_char;
        s
    }

    #[test]
    fn context_opens_on_at_and_closes_on_space() {
        let s = state_with(". fix @her", 10);
        let (at, filter) = s.mention_context().expect("open");
        assert_eq!(at, 6);
        assert_eq!(filter, "her");

        // Completed token (space after) → closed.
        let s = state_with(". fix @her ", 11);
        assert!(s.mention_context().is_none());
    }

    #[test]
    fn update_keeps_popup_in_sync_with_the_caret() {
        let mut s = state_with(". fix @", 7);
        s.update_mention_popup();
        assert!(s.mention.is_some());

        s.cursor_char = 5; // moved before the '@'
        s.update_mention_popup();
        assert!(s.mention.is_none());
    }

    #[test]
    fn matches_come_from_the_smart_input_engine() {
        let ps = projects();
        let s = state_with(". fix @p", 8);
        let texts: Vec<String> = mention_matches(&s, &ps)
            .iter()
            .map(|s| s.insert_text.clone())
            .collect();
        // "projekty" also contains 'p' — the smart input matches both.
        assert_eq!(texts, vec!["@pi-spai", "@\"mojek projekty\""]);

        // Names with spaces insert the quoted form (smart-input rule).
        let s = state_with(". fix @mojek", 12);
        let texts: Vec<String> = mention_matches(&s, &ps)
            .iter()
            .map(|s| s.insert_text.clone())
            .collect();
        assert_eq!(texts.first().map(String::as_str), Some("@\"mojek projekty\""));
    }

    #[test]
    fn accept_replaces_token_like_apply_at_completion() {
        let ps = projects();
        let mut s = state_with(". fix @her", 10);
        s.update_mention_popup();
        assert!(s.accept_mention(&ps));
        assert_eq!(s.lines[0].text, ". fix @herdr ");
        assert_eq!(s.cursor_char, 13);
        assert!(s.mention.is_none());
        assert!(s.dirty);
    }

    #[test]
    fn accept_a_quoted_name_inserts_the_quoted_form() {
        let ps = projects();
        let mut s = state_with(". fix @mojek", 12);
        s.update_mention_popup();
        assert!(s.accept_mention(&ps));
        assert_eq!(s.lines[0].text, ". fix @\"mojek projekty\" ");
    }

    #[test]
    fn selection_wraps_around_like_the_smart_input() {
        let ps = projects();
        let mut s = state_with(". fix @work", 11);
        s.update_mention_popup(); // filter 'work' matches all three via path
        assert_eq!(s.mention.as_ref().unwrap().selected, 0);

        s.mention_move_selection(&ps, -1); // wraps to the last
        assert_eq!(s.mention.as_ref().unwrap().selected, 2);
        s.mention_move_selection(&ps, 1); // wraps back to the first
        assert_eq!(s.mention.as_ref().unwrap().selected, 0);
    }

    #[test]
    fn saved_lines_never_open_the_popup() {
        let mut s = state_with(". fix @her", 10);
        s.lines[0].origin = LineOrigin::Saved {
            path: PathBuf::from("x.md"),
            id: "SPAI-001".to_string(),
            project: "p".to_string(),
            saved_text: ". fix @her".to_string(),
        };
        assert!(s.mention_context().is_none());
        s.update_mention_popup();
        assert!(s.mention.is_none());
    }
}
