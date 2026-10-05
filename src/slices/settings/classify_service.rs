//! Real OpenRouter 5D AI facet classification service for SPAI notes.
//!
//! Ported directly from mozek_rust (`shared/enrichment.rs`):
//! - Injects existing taxonomy (tags, projects, areas) so AI doesn't hallucinate new variants
//! - Temporal expression & deadline resolution from note creation anchor date
//! - Full 5D facet classification (area, effort, urgency, who, project, deadline, priority)
use crate::slices::spai_notes::discovery::SpaiProjectSummary;
use crate::slices::spai_notes::note::SpaiNoteItem;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::process::Command;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Taxonomy {
    pub tags: Vec<String>,
    pub projects: Vec<String>,
    pub areas: Vec<String>,
}

/// Collects all unique existing tags, projects, and area facets across discovered projects.
pub fn collect_taxonomy(projects: &[SpaiProjectSummary]) -> Taxonomy {
    let mut tags = HashSet::new();
    let mut projs = HashSet::new();
    let mut areas = HashSet::new();

    for p in projects {
        projs.insert(p.name.clone());
        for item in &p.items {
            for tag in &item.tags {
                tags.insert(tag.clone());
            }
            if let Some(proj) = &item.facets.project {
                projs.insert(proj.clone());
            }
            if let Some(area) = &item.facets.area {
                areas.insert(area.clone());
            }
        }
    }

    let mut tag_list: Vec<String> = tags.into_iter().collect();
    tag_list.sort();
    let mut proj_list: Vec<String> = projs.into_iter().collect();
    proj_list.sort();
    let mut area_list: Vec<String> = areas.into_iter().collect();
    area_list.sort();

    Taxonomy {
        tags: tag_list,
        projects: proj_list,
        areas: area_list,
    }
}

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

#[derive(Deserialize, Debug, Default)]
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
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub facets: Option<ParsedFacets>,
}

fn build_classification_prompt(item: &SpaiNoteItem, taxonomy: Option<&Taxonomy>) -> String {
    let taxonomy_section = if let Some(tax) = taxonomy {
        let tags_str = if tax.tags.is_empty() { "žádné".to_string() } else { tax.tags.join(", ") };
        let projs_str = if tax.projects.is_empty() { "žádné".to_string() } else { tax.projects.join(", ") };
        let areas_str = if tax.areas.is_empty() { "žádné".to_string() } else { tax.areas.join(", ") };

        format!(
            "\nDŮLEŽITÉ PRO TAXONOMICKOU KONZISTENCI (mozek):\n\
Zde je seznam již existujících štítků (tags), projektů (project) a oblastí (area) v systému uživatele.\n\
Při generování metadat prioritně vybírej z těchto existujících seznamů.\n\
Nové štítky, projekty nebo oblasti vymysli a přidej pouze v případě, že žádný z existujících neodpovídá obsahu.\n\
\n\
Existující štítky (tags): {}\n\
Existující projekty (project): {}\n\
Existující oblasti (area): {}\n",
            tags_str, projs_str, areas_str
        )
    } else {
        String::new()
    };

    format!(
        "Analyzuj následující text poznámky/úkolu a vygeneruj pro ni strukturovaná 5D metadata ve formátu JSON.\n\
Obsah záznamu:\n\
\"\"\"\n\
Titulek: {}\n\
Typ: {}\n\
Stav: {}\n\
Tagy: {}\n\
Datum: {}\n\
Tělo:\n\
{}\n\
\"\"\"\n\
{}\n\
Vrať pouze čistý JSON objekt s následující strukturou:\n\
{{\n\
  \"title\": \"Stručný výstižný název poznámky\",\n\
  \"description\": \"Jednověté shrnutí obsahu\",\n\
  \"tags\": [\"tag1\", \"tag2\"],\n\
  \"facets\": {{\n\
    \"area\": \"oblast/kategorie (např. Frontend, Backend, Architecture, DevOps, Database, Docs, Ops, Notes, Tasks), nebo null\",\n\
    \"effort\": \"odhad náročnosti: low, medium, high, nebo null\",\n\
    \"urgency\": \"naléhavost: low, medium, high, critical, nebo null\",\n\
    \"who\": \"role/řešitel (např. Lead, Developer, Agent, User, External), nebo null\",\n\
    \"project\": \"název projektu, ke kterému záznam patří, nebo null\",\n\
    \"priority\": \"high, medium, low nebo null\",\n\
    \"deadline\": \"YYYY-MM-DD nebo YYYY-MM-DDTHH:mm, nebo null\"\n\
  }}\n\
}}",
        item.title,
        item.kind.as_str(),
        item.status.as_str(),
        item.tags.join(", "),
        item.timestamp,
        item.body,
        taxonomy_section
    )
}

const SYSTEM_PROMPT: &str =
    "Jsi asistent pro správu osobních znalostí, úkolů a poznámek SPAI. Odpovídáš výhradně ve formátu JSON.";

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

    if let Ok(enrichment) = serde_json::from_str::<ModelEnrichmentResponse>(content_str) {
        if let Some(facets) = enrichment.facets {
            return Ok(facets);
        }
    }

    let parsed: ParsedFacets = serde_json::from_str(content_str)
        .map_err(|e| format!("Neplatný JSON od AI modelu: {} ({})", e, content_str))?;

    Ok(parsed)
}
