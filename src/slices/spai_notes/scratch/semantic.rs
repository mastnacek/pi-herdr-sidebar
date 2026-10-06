//! Semantic filter (`Ctrl+R`): on **Enter** — one query embedding + cosine
//! over the stored vectors of the loaded records, run in a background thread
//! with live progress (the same spinner + `█/░` bar the Settings tab shows
//! during vectorization). Never automatic.
//!
//! The result is the *allow-set keyed by file path* — stable across buffer
//! edits (indices shift the moment a line is added or removed, which made the
//! filtered list visibly shift).
use super::super::similarity::cosine_similarity;
use super::super::state::SpaiNotesState;
use super::line_model::LineOrigin;
use super::state::{SemanticJob, SemanticMessage};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc;

impl SpaiNotesState {
    /// Starts the semantic search for `query` in a background thread:
    /// 1) embedding request (network), 2) cosine over loaded record vectors.
    /// Progress arrives through `scratch.poll_semantic()` (tick-driven).
    pub fn scratch_semantic_filter(
        &mut self,
        query: &str,
        api_key: &str,
        model: &str,
        threshold: f64,
    ) {
        if query.trim().len() < 3 {
            self.scratch.last_summary =
                Some("⚠ Dotaz je příliš krátký (min. 3 znaky)".to_string());
            return;
        }
        if api_key.trim().is_empty() {
            self.scratch.last_summary = Some(
                "⚠ Chybí API klíč — nastavte jej v Nastavení (bez klíče jen místní hledání)"
                    .to_string(),
            );
            return;
        }

        // Snapshot the candidates (path/id/project + stored vector) so the
        // thread does not borrow self.
        let mut candidates: Vec<(PathBuf, String, Vec<f64>)> = Vec::new();
        for proj in &self.projects {
            let Some(store) = crate::slices::settings::load_project_vectors(&proj.path) else {
                continue;
            };
            for line in &self.scratch.lines {
                let (path, id, project) = match &line.origin {
                    LineOrigin::Saved { path, id, project, .. }
                    | LineOrigin::FromFile { path, id, project, .. } => {
                        (path.clone(), id.clone(), project.clone())
                    }
                    LineOrigin::New => continue,
                };
                if project != proj.name {
                    continue;
                }
                let file_name = path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(v) = store.vectors.get(&file_name).or_else(|| store.vectors.get(&id)) {
                    candidates.push((path, id, v.clone()));
                }
            }
        }
        // One candidate per file.
        candidates.dedup_by(|a, b| a.0 == b.0);

        let (tx, rx) = mpsc::channel::<SemanticMessage>();
        let total = candidates.len();
        self.scratch.semantic_job = Some(SemanticJob {
            receiver: rx,
            progress: super::state::SemanticProgress::Running {
                step: 0,
                total,
                label: "Sémantické hledání…".to_string(),
            },
            spinner_tick: 0,
        });

        let query = query.to_string();
        let api_key = api_key.to_string();
        let model = model.to_string();
        std::thread::spawn(move || {
            let _ = tx.send(SemanticMessage::Progress {
                step: 0,
                total,
                label: format!("Embedding dotazu ({})…", model),
            });

            let vecs = match crate::slices::settings::vector_service::request_embeddings(
                &api_key,
                &model,
                &[&query],
            ) {
                Ok(v) => v,
                Err(e) => {
                    let _ = tx.send(SemanticMessage::Error(e));
                    return;
                }
            };
            let Some(qvec) = vecs.first() else {
                let _ = tx.send(SemanticMessage::Error(
                    "Embedding se nepodařilo spočítat".to_string(),
                ));
                return;
            };

            // Cosine over the stored vectors, progress per record.
            let mut allowed: HashSet<PathBuf> = HashSet::new();
            for (i, (path, _id, v)) in candidates.iter().enumerate() {
                if cosine_similarity(qvec, v) >= threshold {
                    allowed.insert(path.clone());
                }
                let _ = tx.send(SemanticMessage::Progress {
                    step: i + 1,
                    total,
                    label: format!("Sémantické hledání „{}“", query),
                });
            }

            let hits = candidates.iter().filter(|(p, _, _)| allowed.contains(p)).count();
            let _ = tx.send(SemanticMessage::Done {
                allowed,
                label: format!("Sémantický filtr: {} shod", hits),
            });
        });
    }
}
