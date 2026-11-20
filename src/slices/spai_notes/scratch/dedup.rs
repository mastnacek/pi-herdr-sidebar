//! Ctrl+D in the Scratchpad: on-demand dedup for the line under the cursor
//! (plan §3). Popup anchored under that line; ↑/↓ select, Ctrl+O/A/U act,
//! Esc closes. No timers, no automatic requests.
use crate::shared::spai_client;
use super::super::note_writer;
use super::super::state::SpaiNotesState;
use super::super::storage_format::format_spai_markdown;

impl SpaiNotesState {
    /// Runs `spai similar` for the record under the cursor (Ctrl+D in Edit
    /// mode) in the background. Compares against all projects.
    pub fn run_scratch_dedup(&mut self) {
        let anchor = self.scratch.cursor_line;
        let query = super::line_model::record_text(&self.scratch.lines, anchor);
        let query_trim = super::super::note_writer::without_stamp(query.trim());

        // Reset the popup and anchor it at the current line.
        self.scratch.dedup = super::state::ScratchDedup {
            visible: true,
            anchor_line: anchor,
            ..Default::default()
        };

        if query_trim.len() < 3 {
            return; // panel shows "nothing to compare"
        }
        let proj = note_writer::route_mention(&query_trim, &self.projects)
            .or_else(|| {
                note_writer::current_project(self.current_project_path.as_deref(), &self.projects)
            })
            .or_else(|| self.projects.get(self.selected_project_idx));
        let Some(proj) = proj else {
            return;
        };
        let (cfg, path) = (self.core.clone(), proj.path.clone());
        self.scratch.dedup.is_evaluating = true;
        self.scratch.dedup.receiver = Some(spai_client::spawn(move || {
            spai_client::similar(&cfg, &path, &query_trim)
        }));
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

    fn scratch_dedup_target(&mut self) -> Option<(usize, usize)> {
        let hit = self.scratch.dedup.matches.get(self.scratch.dedup.selected)?.clone();
        let proj_idx = self.projects.iter().position(|p| p.name == hit.project)?;
        self.projects[proj_idx].ensure_items();
        let pos = self.projects[proj_idx]
            .items
            .iter()
            .position(|it| it.id == hit.id)?;
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
        let text_to_append =
            super::line_model::record_text(&self.scratch.lines, self.scratch.dedup.anchor_line)
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

    /// Ctrl+U in the popup: cycle the status of the selected record (`spai set`).
    pub fn scratch_dedup_cycle_status(&mut self) -> Result<String, String> {
        let (proj_idx, pos) = self
            .scratch_dedup_target()
            .ok_or_else(|| "Není vybrána shoda".to_string())?;
        let item = &self.projects[proj_idx].items[pos];
        let (next, id, file, path) = (
            item.status.next_cycle(),
            item.id.clone(),
            item.file_path.clone(),
            self.projects[proj_idx].path.clone(),
        );
        let done = self.set_status_via_spai(&path, &id, &file, next)?;
        Ok(done.as_str().to_string())
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
