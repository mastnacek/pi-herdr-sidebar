//! Ctrl+S pipeline (plan §2): route, allocate, write, switch lines to Saved.
//!
//! Every new marked record becomes a physical file: project = first matching
//! `@mention`, else the current project; a line with no resolvable project
//! stays in the scratchpad with an error (nothing is lost). One failing line
//! never blocks the others. `u` in Read mode deletes the last batch.
use super::line_model::{LineOrigin, ScratchLine};
use super::super::note_writer;
use super::super::state::SpaiNotesState;
use std::path::PathBuf;

impl SpaiNotesState {
    /// Saves every new marked record. Returns the count written.
    /// Ctrl+S never blocks; per-line failures are collected into the summary.
    pub fn scratch_save_all(&mut self) -> usize {
        // Collect record spans first (indices shift as groups collapse).
        let mut records: Vec<(usize, usize, String)> = Vec::new();
        for &start in super::line_model::record_starts(&self.scratch.lines).iter().rev() {
            let line = &self.scratch.lines[start];
            if !line.origin.is_editable() || !super::line_model::is_marked(&line.text) {
                continue;
            }
            let (s, e) = super::line_model::record_span(&self.scratch.lines, start);
            let text = self.scratch.lines[s..e]
                .iter()
                .map(|l| l.text.clone())
                .collect::<Vec<_>>()
                .join("\n");
            records.push((s, e, text));
        }

        if records.is_empty() {
            self.scratch.last_summary = Some("Nic k uložení — žádný označený řádek".to_string());
            return 0;
        }

        // Fresh dedup warning (footer only, no prompt, plan §3).
        self.scratch.warn_similar = self
            .scratch
            .fresh_similar_id()
            .map(|id| format!("⚠ similar: {}", id));

        let mut saved: Vec<(usize, String, PathBuf, String, String)> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        let mut files: Vec<PathBuf> = Vec::new();

        // Save bottom-up so earlier indices stay valid while groups collapse.
        for &(start, end, ref text) in records.iter() {
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
                    saved.push((start, w.id, w.path, w.project, text.clone()));
                }
                Err(e) => errors.push(format!("Řádek {}: {}", start + 1, e)),
            }
        }

        // Apply the successful saves (reverse order: remove continuations).
        {
            let scratch = &mut self.scratch;
            for (start, id, path, project, text) in saved.iter().rev() {
                let (s, e) = super::line_model::record_span(&scratch.lines, *start);
                for _ in s + 1..e {
                    scratch.lines.remove(s + 1);
                }
                scratch.lines[s] = ScratchLine {
                    text: scratch.lines[s].text.clone(),
                    origin: LineOrigin::Saved {
                        path: path.clone(),
                        id: id.clone(),
                        project: project.clone(),
                        saved_text: text.clone(),
                    },
                };
            }
        }

        // Refresh the notes-slice caches so Notes/All scopes see the files.
        let cwd = self.current_project_path.clone();
        self.refresh(cwd.as_deref(), true);

        // Summary: `Uloženo 3 · herdr SPAI-014, SPAI-015 · pi-spai SPAI-022`.
        let count = saved.len();
        let mut by_project: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for (_, id, _, project, _) in saved.iter() {
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
        if !errors.is_empty() {
            summary.push_str(&format!(" · {} chyb", errors.len()));
            if let Some(first) = errors.first() {
                summary.push_str(&format!(" | {}", first));
            }
        }
        self.scratch.last_summary = Some(summary);

        // Undo bookkeeping.
        self.scratch.last_batch = files;
        self.scratch.dirty = count == 0;

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
            // Revert the matching line to the whole original record group
            // (Saved → New; continuation lines come back as their own lines).
            let mut restored: Option<(usize, Vec<String>)> = None;
            for (i, line) in self.scratch.lines.iter().enumerate() {
                if let LineOrigin::Saved {
                    path: p,
                    saved_text,
                    ..
                } = &line.origin
                {
                    if p == path {
                        restored = Some((
                            i,
                            saved_text
                                .split('\n')
                                .map(String::from)
                                .collect(),
                        ));
                        break;
                    }
                }
            }
            if let Some((i, group)) = restored {
                let new_lines: Vec<ScratchLine> = group
                    .into_iter()
                    .map(|text| ScratchLine {
                        text,
                        origin: LineOrigin::New,
                    })
                    .collect();
                let count = new_lines.len();
                self.scratch.lines.splice(i..i + 1, new_lines);
                let _ = count;
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