//! Real OpenRouter embeddings vectorization service for SPAI notes.
use super::state::AsyncProgress;
use crate::slices::spai_notes::note::SpaiNoteItem;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: Vec<&'a str>,
}

#[derive(Deserialize)]
struct EmbeddingItem {
    embedding: Vec<f64>,
    #[serde(default)]
    index: usize,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    #[serde(default)]
    data: Vec<EmbeddingItem>,
    #[serde(default)]
    error: Option<OpenRouterError>,
}

#[derive(Deserialize)]
struct OpenRouterError {
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVectorStore {
    pub version: u32,
    pub model: String,
    pub updated_at: String,
    pub vectors: HashMap<String, Vec<f64>>,
}

/// Calls OpenRouter `/v1/embeddings` with a batch of text inputs using stdin.
pub fn request_embeddings(
    api_key: &str,
    model: &str,
    inputs: &[&str],
) -> Result<Vec<Vec<f64>>, String> {
    if api_key.trim().is_empty() {
        return Err("Chybí OpenRouter API klíč! Nastavte jej v záložce Nastavení (6 -> e)".to_string());
    }

    let payload = serde_json::to_string(&EmbeddingRequest {
        model,
        input: inputs.to_vec(),
    })
    .map_err(|e| e.to_string())?;

    let auth_header = format!("Authorization: Bearer {}", api_key.trim());

    let mut child = Command::new("curl")
        .args([
            "-s",
            "-m",
            "30",
            "-X",
            "POST",
            "https://openrouter.ai/api/v1/embeddings",
            "-H",
            "Content-Type: application/json",
            "-H",
            &auth_header,
            "-H",
            "HTTP-Referer: https://github.com/mastnacek/pi-herdr-sidebar",
            "-H",
            "X-Title: Pi Herdr Sidebar SPAI",
            "-d",
            "@-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Chyba při spouštění curl: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.as_bytes());
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Chyba při čekání na curl: {}", e))?;

    if !output.status.success() {
        return Err("Nepodařilo se připojit k OpenRouter API".to_string());
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let resp: EmbeddingResponse = serde_json::from_str(&body)
        .map_err(|e| format!("Nečitelná odpověď z OpenRouter embeddings: {} ({})", e, body))?;

    if let Some(err) = resp.error {
        return Err(format!("OpenRouter API: {}", err.message));
    }

    let mut result: Vec<Option<Vec<f64>>> = vec![None; inputs.len()];
    for item in resp.data {
        if item.index < result.len() {
            result[item.index] = Some(item.embedding);
        }
    }

    let mut final_vecs = Vec::with_capacity(inputs.len());
    for (i, maybe_vec) in result.into_iter().enumerate() {
        if let Some(v) = maybe_vec {
            final_vecs.push(v);
        } else {
            return Err(format!("Chybí embedding pro položku {}", i));
        }
    }

    Ok(final_vecs)
}

/// Computes vectors for items in a project with live background progress reports.
/// `force_all`: When false, only embeds records missing from `.vectors.json`.
pub fn vectorize_project_items_with_progress(
    api_key: &str,
    model: &str,
    project_dir: &Path,
    items: &[SpaiNoteItem],
    force_all: bool,
    progress_tx: &Sender<AsyncProgress>,
    processed_count: &mut usize,
    total_records: usize,
) -> Result<usize, String> {
    if items.is_empty() {
        return Ok(0);
    }

    let mut vector_map = load_project_vectors(project_dir)
        .map(|s| s.vectors)
        .unwrap_or_default();

    let mut to_fetch_ids = Vec::new();
    let mut to_fetch_texts = Vec::new();

    for item in items {
        if force_all
            || (!vector_map.contains_key(&item.file_name) && !vector_map.contains_key(&item.id))
        {
            let text = format!("{}: {} [{}]", item.kind.as_str(), item.title, item.tags.join(" "));
            to_fetch_ids.push(item.file_name.clone());
            to_fetch_texts.push(text);
        }
    }

    if to_fetch_ids.is_empty() {
        return Ok(0);
    }

    let batch_size = 16;
    let mut newly_indexed = 0;

    for (chunk_ids, chunk_texts) in to_fetch_ids.chunks(batch_size).zip(to_fetch_texts.chunks(batch_size)) {
        let _ = progress_tx.send(AsyncProgress::Progress {
            step: *processed_count,
            total: total_records,
            label: format!("Vektorizuji: {}/{} záznamů (model: {})", *processed_count, total_records, model),
        });

        let refs: Vec<&str> = chunk_texts.iter().map(|s| s.as_str()).collect();
        let computed = request_embeddings(api_key, model, &refs)?;

        for (id, vec) in chunk_ids.iter().zip(computed) {
            vector_map.insert(id.clone(), vec);
            newly_indexed += 1;
            *processed_count += 1;
        }
    }

    let store = ProjectVectorStore {
        version: 1,
        model: model.to_string(),
        updated_at: format!("{:?}", std::time::SystemTime::now()),
        vectors: vector_map,
    };

    save_project_vectors(project_dir, &store)?;
    Ok(newly_indexed)
}

fn vectors_file_path(project_dir: &Path) -> PathBuf {
    let docs_spai = project_dir.join("docs").join("spai");
    if docs_spai.exists() {
        docs_spai.join(".vectors.json")
    } else {
        project_dir.join(".vectors.json")
    }
}

pub fn load_project_vectors(project_dir: &Path) -> Option<ProjectVectorStore> {
    let path = vectors_file_path(project_dir);
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_project_vectors(project_dir: &Path, store: &ProjectVectorStore) -> Result<(), String> {
    let path = vectors_file_path(project_dir);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Chyba při serializaci vektorů: {}", e))?;
    std::fs::write(&path, json)
        .map_err(|e| format!("Chyba při zápisu .vectors.json: {}", e))?;
    Ok(())
}
