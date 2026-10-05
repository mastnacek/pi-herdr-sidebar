//! Filter helpers and Read-mode record navigation (plan §4).
use super::filter::{FilterQuery, StatusFilter};
use super::state::ScratchState;

impl ScratchState {
    /// Indices of records that pass the active filters.
    pub fn visible_records(&self) -> Vec<usize> {
        let default_q = FilterQuery::default();
        let q = self.filter.as_ref().unwrap_or(&default_q);
        super::line_model::record_starts(&self.lines)
            .into_iter()
            .filter(|&idx| {
                super::filter::matches(
                    &self.lines,
                    idx,
                    q,
                    self.status_filter,
                    self.semantic_allowed.as_deref(),
                )
            })
            .collect()
    }

    pub fn visible_count(&self) -> usize {
        self.visible_records().len()
    }

    /// Is the record containing `line_idx` hidden by the active filters?
    pub fn record_hidden(&self, line_idx: usize) -> bool {
        let default_q = FilterQuery::default();
        let q = self.filter.as_ref().unwrap_or(&default_q);
        !super::filter::matches(
            &self.lines,
            line_idx,
            q,
            self.status_filter,
            self.semantic_allowed.as_deref(),
        )
    }

    /// Applies (or clears, `None`) the `/` filter tokens.
    pub fn apply_filter_text(&mut self, text: Option<String>) {
        self.filter = text.map(|t| FilterQuery::parse(&t));
        if self.filter.as_ref().is_some_and(|q| q.is_empty()) {
            self.filter = None;
        }
        self.jump_to_first_visible();
    }

    /// `f`/`F`: cycle the status filter open → all → done.
    pub fn cycle_status_filter(&mut self) {
        self.status_filter = self.status_filter.next();
        self.jump_to_first_visible();
    }

    fn jump_to_first_visible(&mut self) {
        let records = self.visible_records();
        if records.is_empty() {
            return;
        }
        if self.record_hidden(self.cursor_line) {
            self.cursor_line = records[0];
            self.cursor_char = 0;
        }
    }

    /// Moves the cursor by whole records (Read mode `↑/↓`, `j/k`).
    pub fn move_cursor_by_record(&mut self, delta: i32) {
        let records = self.visible_records();
        if records.is_empty() {
            return;
        }
        let pos = records
            .iter()
            .position(|&r| r >= self.cursor_line)
            .unwrap_or(records.len() - 1);
        let new_pos = (pos as i32 + delta).clamp(0, records.len() as i32 - 1) as usize;
        self.cursor_line = records[new_pos];
        self.cursor_char = self.cursor_char.min(self.current_line_len());
    }

    /// `g`/`G`: first/last visible record.
    pub fn jump_to_edge(&mut self, top: bool) {
        let records = self.visible_records();
        let Some(target) = (if top {
            records.first().copied()
        } else {
            records.last().copied()
        }) else {
            return;
        };
        self.cursor_line = target;
        self.cursor_char = 0;
    }
}