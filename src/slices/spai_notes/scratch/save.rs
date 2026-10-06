//! Ctrl+S pipeline (plan §2): route, allocate, write, switch lines to Saved.
//!
//! Every new marked record becomes a physical file: project = first matching
//! `@mention`, else the current project; a line with no resolvable project
//! stays in the scratchpad with an error (nothing is lost). One failing line
//! never blocks the others. `u` in Read mode deletes the last batch.
//!
//! Editing is bidirectional: a Saved record whose text has changed since the
//! save is **updated in its file** (same id, same path) instead of creating
//! a duplicate. FromFile records (Project/All scope) are summaries — they are
//! edited through the Notes editor (Enter/o), so Ctrl+S skips them.
use super::line_model::{LineOrigin, ScratchLine};
use super::super::note_writer;
use super::super::state::SpaiNotesState;
use std::path::PathBuf;

impl SpaiNotesState {
    /// Saves every new marked record and updates modified Saved ones.
    /// Returns the number of created files (updates are reported separately).
    /// Ctrl+S never blocks; per-line failures are collected into the summary.
    pub fn scratch_save_all(&mut self) -> usize {
        // Collect record spans first (indices shift as groups collapse).
        let mut creates: Vec<(usize, usize, String)> = Vec::new();
        let mut updates: Vec<(usize, usize, PathBuf, String)> = Vec::new(); // + original text
        for &start in super::line_model::record_starts(&self.scratch.lines).iter().rev() {
            let head = &self.scratch.lines[start];
            if !super::line_model::is_marked(&head.text) {
                continue; // prose line 0
            }
            let (s, e) = super::line_model::record_span(&self.scratch.lines, start);
            let text = self.scratch.lines[s..e]
                .iter()
                .map(|l| l.text.clone())
                .collect::<Vec<_>>()
                .join("\n");
            match &head.origin {
                LineOrigin::New => creates.push((s, e, text)),
                LineOrigin::Saved { path, saved_text, .. } => {
                    // Original full record: every Saved line keeps its own
                    // original text.
                    let original = self.scratch.lines[s..e]
                        .iter()
                        .filter_map(|l| match &l.origin {
                            LineOrigin::Saved { saved_text, .. } => Some(saved_text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if text != original {
                        updates.push((s, e, path.clone(), original));
                    }
                }
                LineOrigin::FromFile { .. } => {} // summaries: edit via the editor
            }
        }

        // Fresh dedup warning (footer only, no prompt, plan §3).
        self.scratch.warn_similar = self
            .scratch
            .fresh_similar_id()
            .map(|id| format!("⚠ similar: {}", id));

        let mut saved: Vec<(usize, String, PathBuf, String)> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        let mut files: Vec<PathBuf> = Vec::new();

        // Save bottom-up so earlier indices stay valid while groups collapse.
        for &(start, end, ref text) in creates.iter() {
            // Route: first matching @mention; an *explicit but unknown* mention
            // keeps the line in the scratchpad (nothing is lost). Without any
            // mention the record falls back to the current project (plan §2.1).
            let routed = note_writer::route_mention(text, &self.projects);
            let proj = match routed {
                Some(p) => Some(p),
                None => {
                    if text.contains('@') {
                        None // explicit mention that matched nothing → stays
                    } else {
                        note_writer::current_project(
                            self.current_project_path.as_deref(),
                            &self.projects,
                        )
                    }
                }
            };

            let Some(proj) = proj else {
                errors.push(format!(
                    "Řádek {} — projekt z @zmínky nenalezen, zůstává v zápisníku",
                    start + 1
                ));
                continue;
            };

            match note_writer::write_record(&proj, text) {
                Ok(w) => {
                    files.push(w.path.clone());
                    saved.push((start, w.id, w.path, w.project));
                }
                Err(e) => errors.push(format!("Řádek {}: {}", start + 1, e)),
            }
        }

        // Apply the successful creates (reverse order: remove continuations).
        {
            let scratch = &mut self.scratch;
            for (start, id, path, project) in saved.iter().rev() {
                let (s, e) = super::line_model::record_span(&scratch.lines, *start);
                // Continuation lines stay visible as Saved lines of the same
                // file (each keeps its own original text for `u`).
                for i in s..e {
                    let own = scratch.lines[i].text.clone();
                    scratch.lines[i].origin = LineOrigin::Saved {
                        path: path.clone(),
                        id: id.clone(),
                        project: project.clone(),
                        saved_text: own,
                    };
                }
            }
        }

        // Apply the updates (reverse order; spans do not change).
        let mut updated = 0usize;
        for (start, end, path, _original) in updates.iter().rev() {
            let (s, e) = super::line_model::record_span(&self.scratch.lines, *start);
            let text = self.scratch.lines[s..e]
                .iter()
                .map(|l| l.text.clone())
                .collect::<Vec<_>>()
                .join("\n");
            match note_writer::update_record(path, &text) {
                Ok(()) => updated += 1,
                Err(er) => {
                    let _ = end;
                    errors.push(format!("Řádek {}: {}", s + 1, er));
                }
            }
        }

        // Refresh the notes-slice caches so Notes/All scopes see the files.
        let cwd = self.current_project_path.clone();
        self.refresh(cwd.as_deref(), true);

        // Summary: `Uloženo 3 · herdr SPAI-014, SPAI-015 · pi-spai SPAI-022`.
        let count = saved.len();
        let mut by_project: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for (_, id, _, project) in saved.iter() {
            by_project
                .entry(project.clone())
                .or_default()
                .push(id.clone());
        }
        let summary_parts: Vec<String> = by_project
            .into_iter()
            .map(|(p, ids)| format!("{} {}", p, ids.join(", ")))
            .collect();
        let mut summary = format!("Uloženo {}", count);
        if !summary_parts.is_empty() {
            summary.push_str(&format!(" · {}", summary_parts.join(" · ")));
        }
        if updated > 0 {
            summary.push_str(&format!(" · aktualizováno {}", updated));
        }
        if !errors.is_empty() {
            summary.push_str(&format!(" · {} chyb", errors.len()));
            if let Some(first) = errors.first() {
                summary.push_str(&format!(" | {}", first));
            }
        }
        self.scratch.last_summary = Some(summary);

        // Undo bookkeeping (only created files; `u` does not revert updates).
        self.scratch.last_batch = files;
        self.scratch.dirty = false;

        // Bidirectional UX: after a save the cursor must be able to type
        // again — land it on a fresh New line when it sits on a Saved one.
        let cursor_on_saved = matches!(
            self.scratch.lines.get(self.scratch.cursor_line).map(|l| &l.origin),
            Some(LineOrigin::Saved { .. } | LineOrigin::FromFile { .. })
        );
        if cursor_on_saved {
            self.scratch.lines.push(ScratchLine::empty());
            self.scratch.cursor_line = self.scratch.lines.len() - 1;
            self.scratch.cursor_char = 0;
        }

        // Draft of whatever remains unsaved.
        super::draft::save_draft(&self.scratch);

        count
    }

    /// `u` in Read mode: delete the files of the last Ctrl+S batch and revert
    /// the lines back to editable New records.
    pub fn scratch_undo_batch(&mut self) -> Result<usize, String> {
        let paths = self.scratch.last_batch.clone();
        if paths.is_empty() {
            return Err("Žádná dávka k vrácení".to_string());
        }

        let mut undone = 0usize;
        for path in &paths {
            if std::fs::remove_file(path).is_ok() {
                undone += 1;
            }
            // Revert every Saved line of this file back to New with its
            // original text (head and continuations alike).
            for line in self.scratch.lines.iter_mut() {
                if let LineOrigin::Saved {
                    path: p,
                    saved_text,
                    ..
                } = &line.origin
                {
                    if p == path {
                        line.text = saved_text.clone();
                        line.origin = LineOrigin::New;
                    }
                }
            }
        }

        self.scratch.last_batch.clear();
        self.scratch.last_summary =
            Some(format!("Vráceno {} souborů ( Ctrl+S znovu uloží)", undone));

        let cwd = self.current_project_path.clone();
        self.refresh(cwd.as_deref(), true);
        Ok(undone)
    }
}

#[cfg(test)]
mod tests;
