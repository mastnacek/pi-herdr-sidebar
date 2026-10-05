//! Ctrl+D in the Scratchpad: on-demand dedup for the line under the cursor
//! (plan §3). Popup anchored under that line; ↑/↓ select, Ctrl+O/A/U act,
//! Esc closes. No timers, no automatic requests.
use super::super::dedup_engine;
use super::super::note_writer;
use super::super::state::SpaiNotesState;
use super::super::storage_format::format_spai_markdown;

impl SpaiNotesState {
    /// Runs dedup for the record under the cursor (Ctrl+D in Edit mode).
    /// Query = the record text; items = the routed project's records.
    pub fn run_scratch_dedup(&mut self, api_key: &str, model: &str, threshold: f64) {
        let anchor = self.scratch.cursor_line;
        let query = super::line_model::record_text(&self.scratch.lines, anchor);
        let query_trim = query.trim().to_string();

        // Reset the popup and anchor it at the current line.
        self.scratch.dedup = super::state::ScratchDedup {
            visible: true,
            anchor_line: anchor,
            is_evaluating: false,
            matches: Vec::new(),
            selected: 0,
            receiver: None,
        };

        if query_trim.len() < 3 {
            return; // panel shows "nothing to compare"
        }

        // Compare against the routed (or current) project. Resolve it to an
        // index first, then load its items mutably (ensure_items).
        let proj_idx = note_writer::route_mention(&query_trim, &self.projects)
            .and_then(|p| self.projects.iter().position(|x| x.path == p.path))
            .or_else(|| {
                note_writer::current_project(self.current_project_path.as_deref(), &self.projects)
                    .and_then(|p| self.projects.iter().position(|x| x.path == p.path))
            });

        let Some(proj_idx) = proj_idx else {
            return;
        };
        let (items, proj_path) = {
            let proj = &mut self.projects[proj_idx];
            proj.ensure_items();
            (proj.items.clone(), proj.path.clone())
        };

        let outcome = dedup_engine::run(dedup_engine::DedupRequest {
            query: query_trim,
            items,
            project_path: proj_path,
            api_key: api_key.to_string(),
            model: model.to_string(),
            threshold,
        });

        match outcome {
            dedup_engine::DedupOutcome::Ready(matches) => {
                self.scratch.dedup.matches = matches;
                self.scratch.dedup.is_evaluating = false;
            }
            dedup_engine::DedupOutcome::Running(rx) => {
                self.scratch.dedup.is_evaluating = true;
                self.scratch.dedup.receiver = Some(rx);
            }
        }
    }

    /// `↑/↓` in the dedup popup.
    pub fn scratch_dedup_move(&mut self, down: bool) {
        let count = self.scratch.dedup.matches.len();
        if count == 0 {
            return;
        }
        self.scratch.dedup.selected = if down {
            (self.scratch.dedup.selected + 1) % count
        } else {
            (self.scratch.dedup.selected + count - 1) % count
        };
    }

    fn scratch_dedup_target(&self) -> Option<(usize, usize)> {
        let id = self
            .scratch
            .dedup
            .matches
            .get(self.scratch.dedup.selected)
            .map(|m| m.id.clone())?;
        // The project the anchor line routes to (same rule as the run).
        let query = super::line_model::record_text(&self.scratch.lines, self.scratch.dedup.anchor_line);
        let proj_idx = note_writer::route_mention(&query, &self.projects)
            .and_then(|p| self.projects.iter().position(|x| x.path == p.path))
            .unwrap_or(self.selected_project_idx);
        let pos = self.projects.get(proj_idx)?.items.iter().position(|it| it.id == id)?;
        Some((proj_idx, pos))
    }

    /// Ctrl+O in the popup: open the selected record in the Notes editor.
    pub fn scratch_dedup_open(&mut self) -> Result<(), String> {
        let (proj_idx, pos) = self
            .scratch_dedup_target()
            .ok_or_else(|| "Není vybrána shoda".to_string())?;
        let id = self.projects[proj_idx].items[pos].id.clone();
        let project = self.projects[proj_idx].name.clone();

        // Preserve unsaved lines, close scratch, open the editor.
        super::draft::save_draft(&self.scratch);
        self.scratch.close();
        self.selected_project_idx = proj_idx;
        self.selected_item_idx = pos;
        self.open_edit_dialog();
        self.status_message = Some(format!("Otevřen záznam {} ({})", id, project));
        Ok(())
    }

    /// Ctrl+A in the popup: append the record text to the selected note.
    pub fn scratch_dedup_append(&mut self) -> Result<String, String> {
        let (proj_idx, pos) = self
            .scratch_dedup_target()
            .ok_or_else(|| "Není vybrána shoda".to_string())?;
        let text_to_append = super::line_model::record_text(
            &self.scratch.lines,
            self.scratch.dedup.anchor_line,
        )
        .trim()
        .to_string();
        if text_to_append.is_empty() {
            return Err("Text k připojení je prázdný".to_string());
        }

        let (ts, _) = super::super::time_utils::current_timestamp_and_date();
        let item = &self.projects[proj_idx].items[pos];
        let mut updated = item.clone();
        updated
            .body
            .push_str(&format!("\n\n- [{}] Aktualizace: {}", ts, text_to_append));
        let formatted = format_spai_markdown(&updated);
        super::super::note_io::write_atomic(&updated.file_path, &formatted)
            .map_err(|e| format!("Chyba při zápisu: {}", e))?;
        self.projects[proj_idx].items[pos].body = updated.body;

        let id = updated.id.clone();
        // The appended line is consumed: mark it saved-less and drop it.
        self.scratch_dedup_drop_anchor();
        self.status_message = Some(format!("Připojeno k {}", id));
        Ok(id)
    }

    /// Ctrl+U in the popup: cycle the status of the selected record.
    pub fn scratch_dedup_cycle_status(&mut self) -> Result<String, String> {
        let (proj_idx, pos) = self
            .scratch_dedup_target()
            .ok_or_else(|| "Není vybrána shoda".to_string())?;

        let item = &self.projects[proj_idx].items[pos];
        let mut updated = item.clone();
        updated.status = updated.status.next_cycle();
        updated.symbol = updated.status.symbol().to_string();
        let formatted = format_spai_markdown(&updated);
        super::super::note_io::write_atomic(&updated.file_path, &formatted)
            .map_err(|e| format!("Chyba při zápisu: {}", e))?;

        let next = updated.status;
        self.projects[proj_idx].items[pos].status = next;
        self.projects[proj_idx].items[pos].symbol = updated.symbol;
        self.projects[proj_idx].items[pos].body = updated.body;

        Ok(next.as_str().to_string())
    }

    /// Removes the anchor line's record (used by Ctrl+A after appending).
    fn scratch_dedup_drop_anchor(&mut self) {
        let anchor = self.scratch.dedup.anchor_line;
        let (s, e) = super::line_model::record_span(&self.scratch.lines, anchor);
        for _ in s..e {
            self.scratch.lines.remove(s);
        }
        self.scratch.clamp_cursor();
        self.scratch.close_dedup();
    }
}