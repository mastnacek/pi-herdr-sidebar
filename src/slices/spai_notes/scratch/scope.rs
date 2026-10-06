//! Scope loading (plan §5): New (draft), Project, All.
//!
//! - **New**: empty buffer plus the restored draft.
//! - **Project**: `ensure_items()` for the current project only; lines are
//!   `symbol id title`, newest first (items arrive sorted by timestamp).
//! - **All**: lazily over every project (`ensure_items`), rendered
//!   virtually; the filter runs over what is loaded.
use super::line_model::{LineOrigin, ScratchLine};
use super::state::{ScratchScope, ScratchState};
use super::super::state::SpaiNotesState;

impl SpaiNotesState {
    /// Opens the Scratchpad: restores the draft for the New scope.
    /// Draft loading is skipped in Project/All scopes (loaded records fill
    /// the buffer; the draft is only for the New workflow).
    pub fn open_scratch(&mut self, state_dir: Option<std::path::PathBuf>) {
        self.scratch.open(state_dir);
        self.load_scratch_scope(ScratchScope::New);
    }

    /// Closes the Scratchpad, autosaving the draft of unsaved lines.
    pub fn close_scratch(&mut self) {
        super::draft::save_draft(&self.scratch);
        self.scratch.close();
    }

    /// Switches scope (Tab / Shift+Tab in Read mode). The draft of unsaved
    /// lines is kept on disk; Project/All reload from the notes slice.
    pub fn switch_scratch_scope(&mut self, scope: ScratchScope) {
        // Keep unsaved lines safe before the buffer is replaced.
        super::draft::save_draft(&self.scratch);
        self.load_scratch_scope(scope);
    }

    fn load_scratch_scope(&mut self, scope: ScratchScope) {
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.scope = scope;
        scratch.reset_for_scope();

        match scope {
            ScratchScope::New => {
                let restored = super::draft::load_draft(&mut scratch);
                if restored > 0 {
                    scratch.last_summary =
                        Some(format!("Draft obnoven ({} skupin)", restored));
                }
            }
            ScratchScope::Project => {
                let idx = self.selected_project_idx;
                if let Some(proj) = self.projects.get_mut(idx) {
                    proj.ensure_items();
                    let lines: Vec<ScratchLine> =
                        proj.items.iter().map(item_to_line).collect();
                    scratch.lines = lines;
                }
                // Always offer a place for new lines below the loaded records.
                push_new_slot(&mut scratch);
                // Reveal the draft in the footer so the user can return to New.
                if super::draft::draft_path(scratch.state_dir.as_ref())
                    .map(|p| p.exists())
                    .unwrap_or(false)
                {
                    scratch.last_summary =
                        Some("Draft s neuloženými řádky čeká v oblasti Nové".to_string());
                }
            }
            ScratchScope::All => {
                let mut lines = Vec::new();
                for proj in self.projects.iter_mut() {
                    proj.ensure_items();
                    lines.extend(proj.items.iter().map(item_to_line));
                }
                scratch.lines = lines;
                push_new_slot(&mut scratch);
            }
        }

        // The cursor starts on the FIRST record; the window scrolls as it
        // moves (visible_window keeps it in view).
        scratch.cursor_line = 0;
        scratch.cursor_char = 0;
        scratch.scroll = 0;
        self.scratch = scratch;
    }

    /// Opens a record (Read mode Enter/o) in the Notes integrated editor.
    /// The Scratchpad closes first (draft kept) — the editor is the modal.
    pub fn open_scratch_record(&mut self, line_idx: usize) -> Result<(), String> {
        let (path, id, project): (std::path::PathBuf, String, String) = {
            let Some(line) = self.scratch.lines.get(line_idx) else {
                return Err("Prázdný řádek".to_string());
            };
            let Some(path) = line.origin.file_path() else {
                return Err("Řádek není uložený záznam".to_string());
            };
            let Some(id) = line.origin.record_id() else {
                return Err("Záznam nemá SPAI ID".to_string());
            };
            (
                path.clone(),
                id.to_string(),
                line.origin.project().unwrap_or("").to_string(),
            )
        };

        // Route to the project the record lives in.
        let proj_idx = self
            .projects
            .iter()
            .position(|p| {
                p.name == project
                    || p.items.iter().any(|it| it.file_path == path)
            })
            .unwrap_or(self.selected_project_idx);
        self.selected_project_idx = proj_idx;

        if let Some(proj) = self.projects.get_mut(proj_idx) {
            proj.ensure_items();
            if let Some(pos) = proj.items.iter().position(|it| it.file_path == path || it.id == id)
            {
                self.selected_item_idx = pos;
            } else {
                return Err(format!("Záznam {} nenalezen v projektu {}", id, project));
            }
        }

        // Preserve unsaved lines before handing over to the editor.
        super::draft::save_draft(&self.scratch);
        self.scratch.close();
        self.open_edit_dialog();
        self.status_message = Some(format!("Otevřen záznam {} ({})", id, project));
        Ok(())
    }

    /// `x`/`s` in Read mode: cycle the status of the record under the cursor
    /// and write the file. Unsaved lines are ignored.
    pub fn cycle_scratch_record_status(&mut self, line_idx: usize) -> Result<(), String> {
        // Snapshot the line first: cycle_selected_status mutates self.
        let (path, line_text) = {
            let Some(line) = self.scratch.lines.get(line_idx) else {
                return Err("Prázdný řádek".to_string());
            };
            let Some(path) = line.origin.file_path() else {
                return Err("Uložte záznam (Ctrl+S) — tento řádek není soubor".to_string());
            };
            (path.clone(), line.text.clone())
        };

        let proj_idx = self
            .projects
            .iter()
            .position(|p| p.items.iter().any(|it| it.file_path == path))
            .ok_or_else(|| "Záznam není v načtených projektech".to_string())?;

        let pos = self.projects[proj_idx]
            .items
            .iter()
            .position(|it| it.file_path == path)
            .ok_or_else(|| "Záznam nenalezen".to_string())?;

        self.selected_project_idx = proj_idx;
        self.selected_item_idx = pos;
        let res = self.cycle_selected_status();
        if let Ok(()) = res {
            let prefix = format!("{} ", self.projects[proj_idx].items[pos].symbol);
            let rest = super::super::spai_prefixes::strip_leading_prefix(&line_text)
                .unwrap_or(&line_text);
            if let Some(l) = self.scratch.lines.get_mut(line_idx) {
                l.text = format!("{}{}", prefix, rest.trim_start());
            }
        } else if let Err(e) = &res {
            self.scratch.last_summary = Some(e.clone());
        }
        res
    }

    /// Footer hint: type of the current line + live routing preview (plan §1).
    pub fn scratch_footer_hint(&self) -> (String, String) {
        let line_text = self
            .scratch
            .current_line()
            .map(|l| l.text.clone())
            .unwrap_or_default();
        let detected = super::super::input_highlighter::detect_spai_input(&line_text);

        let target = super::super::note_writer::route_mention(&line_text, &self.projects)
            .map(|p| p.name.clone())
            .or_else(|| {
                super::super::note_writer::current_project(self.current_project_path.as_deref(), &self.projects)
                    .map(|p| p.name.clone())
            })
            .unwrap_or_else(|| "—".to_string());

        (
            format!("{} — {}", detected.short_label, detected.prefix_label),
            format!("→ {}", target),
        )
    }
}

/// `symbol id title` on one line, ready to be re-saved/opened.
fn item_to_line(item: &super::super::note::SpaiNoteItem) -> ScratchLine {
    let text = format!("{} {} {}", item.symbol, item.id, item.title);
    ScratchLine {
        text: text.clone(),
        origin: LineOrigin::FromFile {
            path: item.file_path.clone(),
            id: item.id.clone(),
            project: item
                .facets
                .project
                .clone()
                .unwrap_or_else(|| String::new()),
            loaded_text: text,
        },
    }
}

/// A trailing empty New line so the user can start typing in loaded scopes.
fn push_new_slot(scratch: &mut ScratchState) {
    scratch.lines.push(ScratchLine {
        text: String::new(),
        origin: LineOrigin::New,
    });
    scratch.cursor_line = scratch.lines.len() - 1;
}