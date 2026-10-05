//! Real OpenRouter 5D AI facet classification service for SPAI notes.
use super::classify_prompt::{build_classification_prompt, Taxonomy, SYSTEM_PROMPT};
use crate::slices::spai_notes::note::SpaiNoteItem;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};

pub use super::classify_prompt::collect_taxonomy;

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    max_tokens: u32,
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

#[derive(Deserialize, Debug, Default, Clone, PartialEq)]
pub struct ParsedFacets {
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
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub deadline: Option<String>,
}

#[derive(Deserialize)]
struct ModelEnrichmentResponse {
    #[serde(default)]
    pub facets: Option<ParsedFacets>,
}

/// Robust JSON extractor that strips markdown fences and extracts the outermost object.
pub fn extract_json_block(text: &str) -> &str {
    let s = text.trim();
    let s = match s.strip_prefix("```json").or_else(|| s.strip_prefix("```JSON")) {
        Some(rest) => rest.trim_start(),
        None => s.strip_prefix("```").unwrap_or(s).trim_start(),
    };
    let s = s.strip_suffix("```").unwrap_or(s).trim();

    if let (Some(start), Some(end)) = (s.find('{'), s.rfind('}')) {
        if end >= start {
            return &s[start..=end];
        }
    }
    s
}

/// Lenient facet parser: parses JSON or extracts fields line-by-line if truncated.
pub fn parse_facets_lenient(text: &str) -> Result<ParsedFacets, String> {
    let clean = extract_json_block(text);

    if let Ok(resp) = serde_json::from_str::<ModelEnrichmentResponse>(clean) {
        if let Some(facets) = resp.facets {
            return Ok(facets);
        }
    }

    if let Ok(facets) = serde_json::from_str::<ParsedFacets>(clean) {
        return Ok(facets);
    }

    // Heuristic line extractor fallback for partial or truncated JSON
    let mut facets = ParsedFacets::default();
    let mut found_any = false;

    for line in text.lines() {
        let l = line.trim();
        if l.contains("\"area\"") {
            if let Some(v) = extract_field_value(l) {
                facets.area = Some(v);
                found_any = true;
            }
        } else if l.contains("\"effort\"") {
            if let Some(v) = extract_field_value(l) {
                facets.effort = Some(v);
                found_any = true;
            }
        } else if l.contains("\"urgency\"") {
            if let Some(v) = extract_field_value(l) {
                facets.urgency = Some(v);
                found_any = true;
            }
        } else if l.contains("\"who\"") {
            if let Some(v) = extract_field_value(l) {
                facets.who = Some(v);
                found_any = true;
            }
        } else if l.contains("\"project\"") {
            if let Some(v) = extract_field_value(l) {
                facets.project = Some(v);
                found_any = true;
            }
        } else if l.contains("\"deadline\"") {
            if let Some(v) = extract_field_value(l) {
                facets.deadline = Some(v);
                found_any = true;
            }
        } else if l.contains("\"priority\"") {
            if let Some(v) = extract_field_value(l) {
                facets.priority = Some(v);
                found_any = true;
            }
        }
    }

    if found_any {
        Ok(facets)
    } else {
        Err(format!("Nečitelná odpověď od AI: {}", clean))
    }
}

fn extract_field_value(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() >= 2 {
        let val = parts[1].trim().trim_matches(|c| c == '"' || c == ',' || c == '}' || c == ' ' || c == '\n' || c == '\r');
        if !val.is_empty() && val != "null" {
            return Some(val.to_string());
        }
    }
    None
}

/// Calls OpenRouter Chat/Completions to classify a note into 5D facets with taxonomy consistency.
pub fn classify_note(
    api_key: &str,
    model: &str,
    item: &SpaiNoteItem,
    taxonomy: Option<&Taxonomy>,
) -> Result<ParsedFacets, String> {
    if api_key.trim().is_empty() {
        return Err("Chybí OpenRouter API klíč! Nastavte jej v záložce Nastavení (6 -> e)".to_string());
    }

    let user_prompt = build_classification_prompt(item, taxonomy);

    let req = ChatCompletionRequest {
        model,
        messages: vec![
            ChatMessage {
                role: "system",
                content: SYSTEM_PROMPT,
            },
            ChatMessage {
                role: "user",
                content: &user_prompt,
            },
        ],
        max_tokens: 1500,
        temperature: 0.1,
    };

    let payload = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    let auth_header = format!("Authorization: Bearer {}", api_key.trim());

    let mut child = Command::new("curl")
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

    parse_facets_lenient(content_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_and_fenced_json() {
        let raw = "```json\n{\n  \"facets\": {\n    \"area\": \"Backend\",\n    \"effort\": \"medium\",\n    \"urgency\": \"high\",\n    \"who\": \"Lead\"\n  }\n}\n```";
        let facets = parse_facets_lenient(raw).expect("parsed facets");
        assert_eq!(facets.area.as_deref(), Some("Backend"));
        assert_eq!(facets.effort.as_deref(), Some("medium"));
        assert_eq!(facets.urgency.as_deref(), Some("high"));
        assert_eq!(facets.who.as_deref(), Some("Lead"));
    }

    #[test]
    fn parses_partial_truncated_json() {
        let partial = "{\n  \"title\": \"Doladit ZEN\",\n  \"facets\": {\n    \"area\": \"Architecture\",\n    \"effort\": \"low\"\n";
        let facets = parse_facets_lenient(partial).expect("lenient fallback");
        assert_eq!(facets.area.as_deref(), Some("Architecture"));
        assert_eq!(facets.effort.as_deref(), Some("low"));
    }
}
