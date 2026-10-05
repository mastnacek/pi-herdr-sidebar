//! Shared duplicate-check engine with no ties to any dialog (plan §3, phase 0).
//!
//! Input: a query text + the project to compare against. Output: either ready
//! matches (local fallback) or a background receiver (OpenRouter vectorization).
//! Both the creation dialog and the Scratchpad consume this; nothing runs by
//! itself — the caller decides when to invoke it.
use super::note::SpaiNoteItem;
use super::similarity::{find_similar_notes_hybrid, SimilarNoteMatch};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

pub struct DedupRequest {
    pub query: String,
    pub items: Vec<SpaiNoteItem>,
    pub project_path: PathBuf,
    pub api_key: String,
    pub model: String,
    pub threshold: f64,
}

pub enum DedupOutcome {
    /// Computed synchronously (no API key): local text + stored vectors.
    Ready(Vec<SimilarNoteMatch>),
    /// Running in the background; poll the receiver.
    Running(Receiver<Vec<SimilarNoteMatch>>),
}

impl DedupOutcome {
    /// Matches from a synchronous run; `None` while running in the background.
    pub fn ready(&self) -> Option<Vec<SimilarNoteMatch>> {
        match self {
            DedupOutcome::Ready(m) => Some(m.clone()),
            DedupOutcome::Running(_) => None,
        }
    }
}

/// Runs the hybrid check (text + optional query embedding + stored vectors).
pub fn run(req: DedupRequest) -> DedupOutcome {
    let raw = req.query.trim().to_string();
    if raw.len() < 3 {
        return DedupOutcome::Ready(Vec::new());
    }

    if req.api_key.trim().is_empty() {
        let stored_vectors =
            crate::slices::settings::load_project_vectors(&req.project_path).map(|s| s.vectors);
        let matches = find_similar_notes_hybrid(
            &raw,
            &req.items,
            None,
            stored_vectors.as_ref(),
            req.threshold,
            4,
        );
        return DedupOutcome::Ready(matches);
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let items = req.items;
    let proj_path = req.project_path;
    let key = req.api_key;
    let emb_model = req.model;
    let threshold = req.threshold;

    std::thread::spawn(move || {
        let stored_vectors =
            crate::slices::settings::load_project_vectors(&proj_path).map(|s| s.vectors);

        let cand_vec = match crate::slices::settings::vector_service::request_embeddings(
            &key, &emb_model, &[&raw],
        ) {
            Ok(mut vecs) => vecs.pop(),
            Err(_) => None,
        };

        let matches = find_similar_notes_hybrid(
            &raw,
            &items,
            cand_vec.as_deref(),
            stored_vectors.as_ref(),
            threshold,
            4,
        );

        let _ = tx.send(matches);
    });

    DedupOutcome::Running(rx)
}