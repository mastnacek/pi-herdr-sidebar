//! Deleting a scratchpad record (Ctrl+Del, two-step confirm): removes the
//! backing file from disk and the record lines from the buffer. Works for
//! Saved records and records loaded from a project (Project/All scope).
use super::super::state::SpaiNotesState;

impl SpaiNotesState {
    /// First press arms the delete on the cursor line; the second press on
    /// the same line performs it (file deleted, lines removed). Any other
    /// action leaves `confirm_delete` armed until the cursor moves.
    pub fn scratch_delete_request(&mut self, line_idx: usize) {
        let pending = self.scratch.confirm_delete;
        if pending == Some(line_idx) {
            self.scratch.confirm_delete = None;
            match self.scratch_delete_record(line_idx) {
                Ok(n) => self.scratch.last_summary =
                    Some(format!("Smazáno: {} ({} řádků)", n.0, n.1)),
                Err(e) => self.scratch.last_summary = Some(format!("⚠ {}", e)),
            }
        } else {
            self.scratch.confirm_delete = Some(line_idx);
            self.scratch.last_summary =
                Some("Opravdu smazat? Ctrl+Del znovu (soubor se smaže z disku)".to_string());
        }
    }

    /// Performs the delete: file from disk + record lines from the buffer.
    /// Returns (file name, removed line count).
    fn scratch_delete_record(&mut self, line_idx: usize) -> Result<(String, usize), String> {
        let (path, id, project) = {
            let Some(line) = self.scratch.lines.get(line_idx) else {
                return Err("Prázdný řádek".to_string());
            };
            let Some(path) = line.origin.file_path().cloned() else {
                return Err("Tento řádek není uložený záznam".to_string());
            };
            (
                path,
                line.origin.record_id().unwrap_or("").to_string(),
                line.origin.project().unwrap_or("").to_string(),
            )
        };

        // Delete the file.
        let fname = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| id.clone());
        std::fs::remove_file(&path)
            .map_err(|e| format!("Soubor {} nelze smazat: {}", fname, e))?;

        // Drop stale undo entries pointing at the deleted file.
        self.scratch.last_batch.retain(|p| p != &path);

        // Remove the record group (head + continuations) from the buffer.
        let (s, e) = super::line_model::record_span(&self.scratch.lines, line_idx);
        let removed = e - s;
        self.scratch.lines.drain(s..e);

        // Keep the cursor sane and leave an empty line to keep typing.
        self.scratch.cursor_line = s.min(self.scratch.lines.len().saturating_sub(1));
        self.scratch.cursor_char = self.scratch.cursor_char.min(self.scratch.current_line_len());
        if self.scratch.lines.is_empty() {
            self.scratch.lines.push(super::line_model::ScratchLine::empty());
        }

        // Refresh the notes-slice caches.
        let cwd = self.current_project_path.clone();
        self.refresh(cwd.as_deref(), true);
        self.scratch.dirty = true;
        let _ = project;
        Ok((fname, removed))
    }
}

#[cfg(test)]
mod tests {
    use crate::slices::spai_notes::scratch::line_model::{LineOrigin, ScratchLine};
    use crate::slices::spai_notes::state::SpaiNotesState;
    use std::fs;
    use std::path::PathBuf;

    fn temp_project(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("spai_scrdel_{}_{}", tag, std::process::id()));
        fs::create_dir_all(dir.join("docs").join("spai")).unwrap();
        dir
    }

    fn state_on(project: &PathBuf) -> SpaiNotesState {
        let mut state = SpaiNotesState::new(Some(project.clone()));
        state.selected_project_idx = state
            .projects
            .iter()
            .position(|p| p.path == *project)
            .expect("temp project discovered");
        state
    }

    #[test]
    fn ctrl_del_twice_deletes_file_and_lines() {
        let a = temp_project("del");
        let mut state = state_on(&a);
        state
            .create_quick_note_with_status("smaz", crate::slices::spai_notes::note::SpaiType::Todo, crate::slices::spai_notes::note::SpaiStatus::Todo, ". smaz
")
            .unwrap();
        // Load it into the scratchpad as a FromFile record.
        state.switch_scratch_scope(crate::slices::spai_notes::scratch::state::ScratchScope::Project);
        let file = state.projects[state.selected_project_idx].items[0].file_path.clone();
        assert!(file.exists());

        // First press arms, second deletes.
        state.scratch_delete_request(0);
        assert!(file.exists(), "armed only");
        state.scratch_delete_request(0);
        assert!(!file.exists(), "file deleted");
        assert!(state.scratch.lines.iter().all(|l| l.text != ". SPAI-001 smaz"));

        fs::remove_dir_all(&a).ok();
    }

    #[test]
    fn armed_delete_cancels_on_other_line() {
        let a = temp_project("del2");
        let mut state = state_on(&a);
        state.scratch.lines = vec![ScratchLine::empty()];
        state.scratch_delete_request(0);
        state.scratch.cursor_line = 0;
        state.scratch_delete_request(1); // arm elsewhere → replaces, no delete
        assert_eq!(state.scratch.confirm_delete, Some(1));
        fs::remove_dir_all(&a).ok();
    }

    #[test]
    fn delete_requires_a_file_backed_record() {
        let a = temp_project("del3");
        let mut state = state_on(&a);
        state.scratch.lines = vec![crate::slices::spai_notes::scratch::line_model::ScratchLine {
            text: ". nový řádek".to_string(),
            origin: LineOrigin::New,
        }];
        state.scratch_delete_request(0);
        state.scratch_delete_request(0);
        assert!(
            state
                .scratch
                .last_summary
                .as_deref()
                .unwrap_or("")
                .contains("není uložený"),
            "{:?}",
            state.scratch.last_summary
        );
        fs::remove_dir_all(&a).ok();
    }
}
