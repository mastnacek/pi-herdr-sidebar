//! `@` project autocomplete for the Scratchpad.
//!
//! The popup opens the moment a mention token (`@` + anything up to the next
//! whitespace) is open under the caret; matches come from the live project
//! list so routing (`note_writer::route_mention`) and completion cannot
//! disagree. Matching is case-insensitive on the project name and its path.
use super::state::{MentionPopup, ScratchState};
use crate::slices::spai_notes::discovery::SpaiProjectSummary;
use crate::slices::spai_notes::similarity::normalize_czech;

/// Maximum rows in the popup; the filter narrows further.
const MAX_MATCHES: usize = 8;

/// Projects matching the filter currently being typed in `state`'s mention
/// context, best first (starts-with beats contains).
pub fn mention_matches<'a>(
    state: &'a ScratchState,
    projects: &'a [SpaiProjectSummary],
) -> Vec<&'a SpaiProjectSummary> {
    let filter = state
        .mention_context()
        .map(|(_, f)| normalize_czech(&f))
        .unwrap_or_default();
    if filter.is_empty() {
        return projects.iter().take(MAX_MATCHES).collect();
    }
    let mut starts: Vec<&SpaiProjectSummary> = Vec::new();
    let mut contains: Vec<&SpaiProjectSummary> = Vec::new();
    for p in projects {
        let name = normalize_czech(&p.name);
        let path = normalize_czech(&p.path.to_string_lossy());
        if name.starts_with(&filter) {
            starts.push(p);
        } else if name.contains(&filter) || path.contains(&filter) {
            contains.push(p);
        }
        if starts.len() >= MAX_MATCHES {
            break;
        }
    }
    starts.truncate(MAX_MATCHES);
    starts.extend(contains.into_iter().take(MAX_MATCHES - starts.len()));
    starts
}

impl ScratchState {
    /// The `@token` being typed before the cursor on the current line:
    /// `Some((char index of '@', filter characters))`, or `None`.
    ///
    /// The scan stops at the first whitespace before the cursor, so
    /// `mail@example.com` mid-line does not open the popup unless the caret
    /// sits inside that token (Esc dismisses it if it does).
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

    /// Moves the popup selection; clamped against the live match count.
    pub fn mention_move_selection(
        &mut self,
        projects: &[SpaiProjectSummary],
        delta: i32,
    ) {
        let len = mention_matches(self, projects).len().max(1);
        let Some(popup) = &mut self.mention else {
            return;
        };
        let selected = popup.selected as i32 + delta;
        popup.selected = selected.clamp(0, len as i32 - 1) as usize;
    }

    /// Accepts the selected match, replacing `@filter` with `@name`.
    /// Returns `false` when the popup is closed or nothing matches.
    pub fn accept_mention(&mut self, projects: &[SpaiProjectSummary]) -> bool {
        let selected = match &self.mention {
            Some(p) => p.selected,
            None => return false,
        };
        let matches = mention_matches(self, projects);
        let Some(m) = matches.get(selected) else {
            return false;
        };
        let name = m.name.clone();
        if !self.complete_mention(&name) {
            return false;
        }
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
    fn matches_rank_starts_with_before_contains() {
        let ps = projects();
        let s = state_with(". fix @p", 8);
        let names: Vec<&str> = mention_matches(&s, &ps)
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, vec!["pi-spai"], "only the start-of-name hit");

        // A path-only filter matches through the path.
        let s = state_with(". fix @work", 10);
        let names: Vec<&str> = mention_matches(&s, &ps)
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names.len(), 2, "both paths contain 'work'");
    }

    #[test]
    fn accept_replaces_token_and_closes_the_popup() {
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
    fn selection_survives_refilter_and_is_clamped() {
        let ps = projects();
        let mut s = state_with(". fix @", 7);
        s.update_mention_popup();
        s.mention_move_selection(&ps, 1);
        assert_eq!(s.mention.as_ref().unwrap().selected, 1);
        // Only 2 projects: moving further down clamps to 1.
        s.mention_move_selection(&ps, 5);
        assert_eq!(s.mention.as_ref().unwrap().selected, 1);
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
