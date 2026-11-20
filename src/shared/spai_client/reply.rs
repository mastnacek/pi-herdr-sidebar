//! Answers of `spai --json` as the sidebar uses them.
use serde_json::Value;
use std::path::PathBuf;

/// A record similar to the typed text, from any project.
#[derive(Debug, Clone, PartialEq)]
pub struct SimilarHit {
    pub project: String,
    pub id: String,
    /// Czech title when the record has one, else its title.
    pub title: String,
    pub score: f32,
    pub path: String,
    /// At or above the duplicate threshold (else only related).
    pub duplicate: bool,
}

impl SimilarHit {
    fn from_json(v: &Value, duplicate: bool) -> Self {
        let text = |k: &str| v[k].as_str().unwrap_or_default().to_string();
        Self {
            project: text("project"),
            id: text("id"),
            title: text("title"),
            score: v["score"].as_f64().unwrap_or(0.0) as f32,
            path: text("path"),
            duplicate,
        }
    }

    pub fn percent(&self) -> u32 {
        (self.score * 100.0).round() as u32
    }
}

pub(super) fn hits(data: &Value) -> Vec<SimilarHit> {
    let list = |key: &str, duplicate: bool| -> Vec<SimilarHit> {
        data[key]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|v| SimilarHit::from_json(v, duplicate))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut all = list("duplicates", true);
    all.extend(list("related", false));
    all
}

/// A record `add` wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct Written {
    pub id: String,
    pub project: String,
    pub path: PathBuf,
    pub title_cs: Option<String>,
    pub title: String,
    /// Work still to do (no network): `translate_en`, `embed`, ...
    pub pending: Vec<String>,
    pub warnings: Vec<String>,
    pub related: Vec<SimilarHit>,
}

impl Written {
    /// One status line: id, Czech title, warning when the record waits for the network.
    pub fn notice(&self) -> String {
        let title = self.title_cs.as_deref().unwrap_or(&self.title);
        let mut s = format!("✓ {} · {}", self.id, title);
        if !self.pending.is_empty() {
            s.push_str(&format!(
                " · ⚠ bez sítě, čeká: {} (doplní spai reindex)",
                self.pending.join(", ")
            ));
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AddOutcome {
    Written(Written),
    /// Nothing was written: a similar record exists (duplicates first).
    Duplicates(Vec<SimilarHit>),
}
