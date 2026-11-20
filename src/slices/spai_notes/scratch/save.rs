//! Ctrl+S pipeline (plan §2): route, write through `spai-core`, switch lines to Saved.
//!
//! Every new marked record is written by `spai add` (English storage, Czech
//! title, vector, duplicate check across all projects) on a **background
//! thread**; the footer shows "ukládám…" and the buffer is read-only until the
//! result arrives (`save_finish`). Project = first matching `@mention`, else
//! the current project; a line with no resolvable project stays in the
//! scratchpad with an error (nothing is lost). A duplicate writes nothing: the
//! popup lists the similar records and Ctrl+S in it saves anyway.
//!
//! Editing is bidirectional: a record whose text has changed since it was
//! written is **updated in its file** (same id, same path) instead of creating
//! a duplicate — for Saved records and for FromFile records loaded with
//! Ctrl+T alike. FromFile lines carry the id (`SPAI-014`), which is stripped
//! before the rewrite so the title is never polluted.
use crate::shared::spai_client;
use super::super::note_writer;
use super::super::spai_prefixes::strip_leading_prefix;
use super::super::state::SpaiNotesState;
use super::line_model::LineOrigin;
use super::save_job::{CreateItem, SaveJob};
use std::path::PathBuf;

impl SpaiNotesState {
    /// Saves every new marked record (in the background) and updates modified
    /// Saved ones (at once). The outcome lands in `poll_scratch_save`.
    pub fn scratch_save_all(&mut self) {
        if self.scratch.save_job.is_some() {
            return;
        }
        // Collect record spans first (indices shift as groups collapse).
        let mut creates: Vec<(usize, String)> = Vec::new();
        let mut updates: Vec<(usize, PathBuf)> = Vec::new();
        for &start in super::line_model::record_starts(&self.scratch.lines)
            .iter()
            .rev()
        {
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
                LineOrigin::New => creates.push((s, text)),
                LineOrigin::Saved { path, .. } => {
                    // Every Saved line keeps its own original text.
                    let original = self.scratch.lines[s..e]
                        .iter()
                        .filter_map(|l| match &l.origin {
                            LineOrigin::Saved { saved_text, .. } => Some(saved_text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if text.trim_end() != original.trim_end() {
                        updates.push((s, path.clone()));
                    }
                }
                LineOrigin::FromFile {
                    path, loaded_text, ..
                } => {
                    // Continuations typed under the summary count as changes.
                    if text.trim_end() != loaded_text.trim_end() {
                        updates.push((s, path.clone()));
                    }
                }
            }
        }

        // Fresh dedup warning (footer only, no prompt, plan §3).
        self.scratch.warn_similar = self
            .scratch
            .fresh_similar_id()
            .map(|id| format!("⚠ similar: {}", id));

        let mut errors: Vec<String> = Vec::new();
        let mut items: Vec<CreateItem> = Vec::new();
        for (start, text) in creates {
            match self.route_create(start, &text) {
                Ok(item) => items.push(item),
                Err(e) => errors.push(e),
            }
        }
        let (updated, retitled) = self.apply_scratch_updates(&updates, &mut errors);
        self.start_reindex(retitled);
        self.start_creates(items, false, updated, errors);
    }

    /// Project of a new record: first matching `@mention`; an *explicit but
    /// unknown* mention keeps the line in the scratchpad (nothing is lost);
    /// without any mention the current project (plan §2.1).
    fn route_create(&self, start: usize, text: &str) -> Result<CreateItem, String> {
        let proj = match note_writer::route_mention(text, &self.projects) {
            Some(p) => Some(p),
            None if text.contains('@') => None,
            None => note_writer::current_project(
                self.current_project_path.as_deref(),
                &self.projects,
            ),
        };
        let proj = proj.ok_or_else(|| {
            format!(
                "Řádek {} — projekt z @zmínky nenalezen, zůstává v zápisníku",
                start + 1
            )
        })?;
        Ok(CreateItem {
            start,
            project_path: proj.path.clone(),
            text: note_writer::without_stamp(text),
        })
    }

    /// Starts the background write of `items`; with nothing to write the
    /// summary is built at once.
    pub(super) fn start_creates(
        &mut self,
        items: Vec<CreateItem>,
        force: bool,
        updated: usize,
        errors: Vec<String>,
    ) {
        if items.is_empty() {
            self.finish_scratch_save(updated, errors, Vec::new());
            return;
        }
        let cfg = self.core.clone();
        let work = items.clone();
        let rx = spai_client::spawn(move || {
            work.into_iter()
                .map(|it| {
                    let r = spai_client::add_record(&cfg, &it.project_path, &it.text, force);
                    (it.start, r)
                })
                .collect::<Vec<_>>()
        });
        self.scratch.last_summary = Some("⏳ ukládám…".to_string());
        self.scratch.save_job = Some(SaveJob {
            rx,
            updated,
            errors,
            spinner_tick: 0,
        });
    }

    /// Ctrl+S in the duplicate popup: write the stopped record anyway.
    pub fn scratch_force_save(&mut self) -> Result<(), String> {
        let start = self
            .scratch
            .dedup
            .force_start
            .ok_or_else(|| "Není co uložit i tak".to_string())?;
        let (s, e) = super::line_model::record_span(&self.scratch.lines, start);
        let text = self.scratch.lines[s..e]
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        let item = self.route_create(s, &text)?;
        self.scratch.close_dedup();
        self.start_creates(vec![item], true, 0, Vec::new());
        Ok(())
    }

    /// Applies the edits of already saved records to their files (reverse
    /// order; spans do not change). Returns the number of updated records and
    /// the projects of records that now wait for `retitle_en`.
    fn apply_scratch_updates(
        &mut self,
        updates: &[(usize, PathBuf)],
        errors: &mut Vec<String>,
    ) -> (usize, Vec<PathBuf>) {
        let mut updated = 0usize;
        let mut retitled: Vec<PathBuf> = Vec::new();
        for (start, path) in updates.iter().rev() {
            let (s, e) = super::line_model::record_span(&self.scratch.lines, *start);
            let text = self.scratch.lines[s..e]
                .iter()
                .map(|l| l.text.clone())
                .collect::<Vec<_>>()
                .join("\n");
            // FromFile summary lines carry the id (`. SPAI-014 Název`) — strip
            // it so the id never lands in the title; keep the original mark
            // prefix and any continuation lines.
            let file_text = strip_leading_id(&text);
            match note_writer::update_record(path, &file_text) {
                Ok(retitle) => {
                    updated += 1;
                    // <project>/docs/spai/<file>.md
                    if let (true, Some(root)) = (retitle, path.ancestors().nth(3)) {
                        retitled.push(root.to_path_buf());
                    }
                    // Refresh the change-detection baseline line by line.
                    let new_lines: Vec<String> = file_text.lines().map(String::from).collect();
                    let head_origin = self.scratch.lines[s].origin.clone();
                    for (i, line) in self.scratch.lines[s..e].iter_mut().enumerate() {
                        let own = line.text.clone();
                        match &head_origin {
                            LineOrigin::Saved {
                                path: p,
                                id: id2,
                                project: pr,
                                ..
                            } => {
                                line.origin = LineOrigin::Saved {
                                    path: p.clone(),
                                    id: id2.clone(),
                                    project: pr.clone(),
                                    saved_text: own,
                                };
                            }
                            LineOrigin::FromFile {
                                path: p,
                                id: id2,
                                project: pr,
                                ..
                            } => {
                                line.origin = LineOrigin::FromFile {
                                    path: p.clone(),
                                    id: id2.clone(),
                                    project: pr.clone(),
                                    loaded_text: new_lines.get(i).cloned().unwrap_or(own.clone()),
                                };
                            }
                            LineOrigin::New => {}
                        }
                    }
                }
                Err(er) => errors.push(format!("Řádek {}: {}", s + 1, er)),
            }
        }
        (updated, retitled)
    }
}

/// Strips the `SPAI-xxx ` token that follows the mark prefix on FromFile
/// summary lines (`. SPAI-014 Název` → `. Název`), keeping the original mark
/// prefix and every continuation line.
fn strip_leading_id(text: &str) -> String {
    let (first, rest) = match text.split_once('\n') {
        Some((f, r)) => (f, Some(r)),
        None => (text, None),
    };
    let clean = strip_leading_prefix(first).unwrap_or(first);
    let prefix_len = first.len() - clean.len(); // the mark prefix itself
    let without_id = match clean.split_once(' ') {
        Some((id, title)) if id.starts_with("SPAI-") => title.trim().to_string(),
        _ => clean.trim().to_string(),
    };
    let mut out = String::new();
    out.push_str(&first[..prefix_len]);
    out.push_str(&without_id);
    if let Some(r) = rest {
        out.push('\n');
        out.push_str(r);
    }
    out
}

#[cfg(test)]
mod id_tests {
    use super::*;

    #[test]
    fn strips_the_id_but_not_the_mark_prefix() {
        assert_eq!(
            strip_leading_id(". SPAI-014 Opravit build"),
            ". Opravit build"
        );
        assert_eq!(strip_leading_id("x SPAI-003 hotová věc"), "x hotová věc");
        assert_eq!(
            strip_leading_id("? SPAI-007 nápad\npokračování"),
            "? nápad\npokračování"
        );
        // No id (a Saved record edited into a plain title) stays untouched.
        assert_eq!(strip_leading_id(". bez id"), ". bez id");
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_more;
#[cfg(test)]
mod tests_retitle;
