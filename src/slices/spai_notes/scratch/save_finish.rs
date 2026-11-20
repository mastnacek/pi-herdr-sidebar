//! The end of a Scratchpad save: results of the background `spai add`
//! become Saved lines, a summary, a duplicate popup; plus undo (`u`).
use crate::shared::spai_client::AddOutcome;
use super::super::state::SpaiNotesState;
use super::line_model::{LineOrigin, ScratchLine};
use super::save_job::SaveResults;
use super::state::ScratchDedup;
use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;

impl SpaiNotesState {
    /// Takes a finished background write, if any. Passive: called from the tick.
    pub fn poll_scratch_save(&mut self) {
        let Some(job) = &mut self.scratch.save_job else {
            return;
        };
        job.spinner_tick = job.spinner_tick.wrapping_add(1);
        match job.rx.try_recv() {
            Ok(results) => {
                if let Some(job) = self.scratch.save_job.take() {
                    self.finish_scratch_save(job.updated, job.errors, results);
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                if let Some(mut job) = self.scratch.save_job.take() {
                    job.errors.push("Zápis se nedokončil".to_string());
                    self.finish_scratch_save(job.updated, job.errors, Vec::new());
                }
            }
        }
    }

    /// Waits for the background write (tests only).
    #[cfg(test)]
    pub fn wait_scratch_save(&mut self) {
        if let Some(job) = self.scratch.save_job.take() {
            let results = job.rx.recv().expect("save thread");
            self.finish_scratch_save(job.updated, job.errors, results);
        }
    }

    /// Switches the written lines to Saved, builds the summary, opens the
    /// duplicate popup for the first stopped record.
    pub(super) fn finish_scratch_save(
        &mut self,
        updated: usize,
        mut errors: Vec<String>,
        results: SaveResults,
    ) {
        let mut saved: Vec<(usize, String, PathBuf, String)> = Vec::new();
        let mut files: Vec<PathBuf> = Vec::new();
        let mut offline = 0usize;
        let mut duplicates = 0usize;
        let mut first_dup = None;
        for (start, result) in results {
            match result {
                Ok(AddOutcome::Written(w)) => {
                    if !w.pending.is_empty() {
                        offline += 1;
                    }
                    files.push(w.path.clone());
                    saved.push((start, w.id, w.path, w.project));
                }
                Ok(AddOutcome::Duplicates(hits)) => {
                    duplicates += 1;
                    if first_dup.is_none() {
                        first_dup = Some((start, hits));
                    }
                }
                Err(e) => errors.push(format!("Řádek {}: {}", start + 1, e)),
            }
        }

        // Continuation lines stay visible as Saved lines of the same file
        // (each keeps its own original text for `u`).
        for (start, id, path, project) in saved.iter().rev() {
            let (s, e) = super::line_model::record_span(&self.scratch.lines, *start);
            for i in s..e {
                let own = self.scratch.lines[i].text.clone();
                self.scratch.lines[i].origin = LineOrigin::Saved {
                    path: path.clone(),
                    id: id.clone(),
                    project: project.clone(),
                    saved_text: own,
                };
            }
        }

        // Refresh the notes-slice caches so Notes/All scopes see the files.
        self.reload_written(&files);

        // Summary: `Uloženo 3 · herdr SPAI-014, SPAI-015 · pi-spai SPAI-022`.
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
        let mut summary = format!("Uloženo {}", saved.len());
        if !summary_parts.is_empty() {
            summary.push_str(&format!(" · {}", summary_parts.join(" · ")));
        }
        if updated > 0 {
            summary.push_str(&format!(" · aktualizováno {}", updated));
        }
        if offline > 0 {
            summary.push_str(&format!(
                " · ⚠ bez sítě {} (překlad/vektor doplní spai reindex)",
                offline
            ));
        }
        if let Some((start, hits)) = first_dup {
            summary.push_str(&format!(
                " · ⚠ duplicita {}× — Ctrl+S v okně uloží i tak",
                duplicates
            ));
            self.scratch.cursor_line = start;
            self.scratch.cursor_char = 0;
            self.scratch.dedup = ScratchDedup {
                visible: true,
                anchor_line: start,
                is_evaluating: false,
                matches: hits,
                selected: 0,
                receiver: None,
                force_start: Some(start),
            };
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
        self.scratch.dirty = self
            .scratch
            .lines
            .iter()
            .any(|l| matches!(l.origin, LineOrigin::New) && !l.text.trim().is_empty());

        // Bidirectional UX: after a save the cursor must be able to type
        // again — land it on a fresh New line when it sits on a Saved one.
        let cursor_on_saved = matches!(
            self.scratch
                .lines
                .get(self.scratch.cursor_line)
                .map(|l| &l.origin),
            Some(LineOrigin::Saved { .. } | LineOrigin::FromFile { .. })
        );
        if cursor_on_saved {
            self.scratch.lines.push(ScratchLine::empty());
            self.scratch.cursor_line = self.scratch.lines.len() - 1;
            self.scratch.cursor_char = 0;
        }

        // Draft of whatever remains unsaved.
        super::draft::save_draft(&self.scratch);
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
