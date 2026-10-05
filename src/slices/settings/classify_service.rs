//! Real OpenRouter 5D AI facet classification service for SPAI notes.
use crate::slices::spai_notes::note::SpaiNoteItem;
use crate::slices::spai_notes::storage_format::format_spai_markdown;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ResponseFormat<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
}

#[derive(Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    response_format: ResponseFormat<'a>,
    temperature: f32,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageContent,
}

#[derive(Deserialize)]
struct ChatMessageContent {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
    #[serde(default)]
    error: Option<OpenRouterError>,
}

#[derive(Deserialize)]
struct OpenRouterError {
    message: String,
}

#[derive(Deserialize)]
struct ParsedFacets {
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub urgency: Option<String>,
    #[serde(default)]
    pub who: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
}

/// Calls OpenRouter Chat/Completions to classify a note into 5D facets.
pub fn classify_note(
    api_key: &str,
    model: &str,
    item: &SpaiNoteItem,
) -> Result<ParsedFacets, String> {
    if api_key.trim().is_empty() {
        return Err("Chybí OpenRouter API klíč! Nastavte jej v záložce Nastavení (6 -> e)".to_string());
    }

    let system_prompt = "You are a SPAI 5D Facet Classifier. Classify the note into JSON with keys: \
\"area\" (e.g. Frontend, Backend, Architecture, DevOps, Database, Docs, Ops, Notes, Tasks), \
\"effort\" (\"low\"|\"medium\"|\"high\"), \
\"urgency\" (\"low\"|\"medium\"|\"high\"|\"critical\"), \
\"who\" (\"Lead\"|\"Developer\"|\"Agent\"|\"User\"|\"External\"), \
\"project\" (short slug). Return only valid JSON.";

    let user_content = format!(
        "Title: {}\nType: {}\nStatus: {}\nTags: {}\nBody: {}",
        item.title,
        item.kind.as_str(),
        item.status.as_str(),
        item.tags.join(", "),
        item.body
    );

    let req = ChatCompletionRequest {
        model,
        messages: vec![
            ChatMessage {
                role: "system",
                content: system_prompt,
            },
            ChatMessage {
                role: "user",
                content: &user_content,
            },
        ],
        response_format: ResponseFormat {
            kind: "json_object",
        },
        temperature: 0.1,
    };

    let payload = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    let auth_header = format!("Authorization: Bearer {}", api_key.trim());

    let output = Command::new("curl")
        .args([
            "-s",
            "-m",
            "30",
            "-X",
            "POST",
            "https://openrouter.ai/api/v1/chat/completions",
            "-H",
            "Content-Type: application/json",
            "-H",
            &auth_header,
            "-H",
            "HTTP-Referer: https://github.com/mastnacek/pi-herdr-sidebar",
            "-H",
            "X-Title: Pi Herdr Sidebar SPAI",
            "-d",
            &payload,
        ])
        .output()
        .map_err(|e| format!("Chyba při volání curl: {}", e))?;

    if !output.status.success() {
        return Err("Nepodařilo se připojit k OpenRouter API".to_string());
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let resp: ChatCompletionResponse = serde_json::from_str(&body)
        .map_err(|e| format!("Nečitelná odpověď z OpenRouter chat: {} ({})", e, body))?;

    if let Some(err) = resp.error {
        return Err(format!("OpenRouter API: {}", err.message));
    }

    let content_str = resp
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .ok_or_else(|| "OpenRouter vrátil prázdnou odpověď".to_string())?;

    let parsed: ParsedFacets = serde_json::from_str(content_str)
        .map_err(|e| format!("Neplatný JSON od AI modelu: {} ({})", e, content_str))?;

    Ok(parsed)
}

/// Iterates through notes, classifies them with OpenRouter, and updates their markdown files.
pub fn classify_project_notes(
    api_key: &str,
    model: &str,
    items: &mut [SpaiNoteItem],
    only_unclassified: bool,
) -> Result<usize, String> {
    let mut updated_count = 0;

    for item in items.iter_mut() {
        if only_unclassified && item.facets.area.is_some() && item.facets.effort.is_some() {
            continue;
        }

        let facets = classify_note(api_key, model, item)?;

        if let Some(area) = facets.area {
            item.facets.area = Some(area);
        }
        if let Some(effort) = facets.effort {
            item.facets.effort = Some(effort);
        }
        if let Some(urgency) = facets.urgency {
            item.facets.urgency = Some(urgency);
        }
        if let Some(who) = facets.who {
            item.facets.who = Some(who);
        }
        if let Some(proj) = facets.project {
            if item.facets.project.is_none() {
                item.facets.project = Some(proj);
            }
        }

        let content = format_spai_markdown(item);
        if let Err(e) = std::fs::write(&item.file_path, content) {
            return Err(format!("Chyba při zápisu souboru {}: {}", item.file_path.display(), e));
        }

        updated_count += 1;
    }

    Ok(updated_count)
}
