//! `~` semantic filter (plan §4, phase 4): on **Enter** — one query embedding
//! + cosine over the stored vectors of the loaded records. Never automatic.
use super::super::similarity::cosine_similarity;
use super::super::state::SpaiNotesState;
use super::line_model::LineOrigin;
use std::collections::HashMap;

impl SpaiNotesState {
    /// Computes the allowed set for `~query`: for every loaded record with a
    /// file-backed vector, cosine vs. the query embedding, kept when
    /// `sim >= threshold`. Sets `scratch.semantic_allowed` (AND-composed with
    /// other filter tokens). Returns the hit count.
    pub fn scratch_semantic_filter(
        &mut self,
        query: &str,
        api_key: &str,
        model: &str,
        threshold: f64,
    ) -> Result<usize, String> {
        if query.trim().len() < 3 {
            return Err("Dotaz je příliš krátký (min. 3 znaky)".to_string());
        }

        let vecs = crate::slices::settings::vector_service::request_embeddings(
            api_key, model, &[query],
        )
        .map_err(|e| e)?;
        let Some(qvec) = vecs.first() else {
            return Err("Embedding se nepodařilo spočítat".to_string());
        };
        let qvec = qvec.clone();

        // Vector stores per project, keyed by file name or id.
        let mut stores: HashMap<String, HashMap<String, Vec<f64>>> = HashMap::new();
        for proj in &self.projects {
            if let Some(store) = crate::slices::settings::load_project_vectors(&proj.path) {
                stores.insert(proj.name.clone(), store.vectors);
            }
        }

        let mut allowed: Vec<usize> = Vec::new();
        for (idx, line) in self.scratch.lines.iter().enumerate() {
            let (project, path, id) = match &line.origin {
                LineOrigin::Saved { path, id, project, .. }
                | LineOrigin::FromFile { path, id, project, .. } => {
                    (project.clone(), path.clone(), id.clone())
                }
                LineOrigin::New => continue, // unsaved lines have no vector
            };

            let file_name = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("")
                .to_string();

            let Some(store) = stores.get(&project) else {
                continue;
            };
            let Some(v) = store.get(&file_name).or_else(|| store.get(&id)) else {
                continue;
            };
            if cosine_similarity(&qvec, v) >= threshold {
                allowed.push(idx);
            }
        }

        let hits = allowed.len();
        self.scratch.semantic_allowed = Some(allowed);
        Ok(hits)
    }

    /// Clears the semantic filter (`f`/`F` clear path, or a fresh filter run).
    pub fn scratch_clear_semantic(&mut self) {
        self.scratch.semantic_allowed = None;
    }
}