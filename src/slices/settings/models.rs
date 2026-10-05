//! Dynamic OpenRouter models catalog, pricing, caching, and search.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Parsed and normalized model metadata from OpenRouter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub context_length: u64,
    pub prompt_price_m: f64,     // $ per 1M prompt tokens
    pub completion_price_m: f64, // $ per 1M completion tokens
    pub is_free: bool,
}

impl ModelInfo {
    pub fn price_label(&self) -> String {
        format_price(self.prompt_price_m, self.completion_price_m)
    }

    pub fn context_label(&self) -> String {
        format_context(self.context_length)
    }
}

pub fn format_price(prompt_m: f64, completion_m: f64) -> String {
    if prompt_m == 0.0 && completion_m == 0.0 {
        "FREE".to_string()
    } else {
        format!("${:.2} / ${:.2} /1M", prompt_m, completion_m)
    }
}

pub fn format_context(ctx: u64) -> String {
    if ctx == 0 {
        "-".to_string()
    } else if ctx >= 1_000_000 {
        format!("{}M", ctx / 1_000_000)
    } else if ctx >= 1_000 {
        format!("{}k", ctx / 1_000)
    } else {
        format!("{}", ctx)
    }
}

// ---------------------------------------------------------------------------
// OpenRouter API JSON Wire Model
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct OpenRouterPricingRaw {
    #[serde(default)]
    prompt: Option<serde_json::Value>,
    #[serde(default)]
    completion: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct OpenRouterModelRaw {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    pricing: Option<OpenRouterPricingRaw>,
}

#[derive(Deserialize)]
struct OpenRouterResponseRaw {
    #[serde(default)]
    data: Vec<OpenRouterModelRaw>,
}

fn parse_price_val(val: Option<&serde_json::Value>) -> f64 {
    let Some(v) = val else { return 0.0 };
    if let Some(n) = v.as_f64() {
        return n * 1_000_000.0;
    }
    if let Some(s) = v.as_str() {
        if let Ok(parsed) = s.parse::<f64>() {
            return parsed * 1_000_000.0;
        }
    }
    0.0
}

/// Parses the raw OpenRouter JSON catalog response into `Vec<ModelInfo>`.
pub fn parse_openrouter_json(body: &str) -> Vec<ModelInfo> {
    let Ok(root) = serde_json::from_str::<OpenRouterResponseRaw>(body) else {
        return Vec::new();
    };

    root.data
        .into_iter()
        .map(|m| {
            let prompt_m = m
                .pricing
                .as_ref()
                .map(|p| parse_price_val(p.prompt.as_ref()))
                .unwrap_or(0.0);
            let completion_m = m
                .pricing
                .as_ref()
                .map(|p| parse_price_val(p.completion.as_ref()))
                .unwrap_or(0.0);
            let is_free = prompt_m == 0.0 && completion_m == 0.0;
            let name = m.name.unwrap_or_else(|| m.id.clone());
            let description = m.description.unwrap_or_default();
            let context_length = m.context_length.unwrap_or(0);

            ModelInfo {
                id: m.id,
                name,
                description,
                context_length,
                prompt_price_m: prompt_m,
                completion_price_m: completion_m,
                is_free,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Network Fetch & Cache
// ---------------------------------------------------------------------------

const FETCH_TIMEOUT_SECS: u32 = 8;

pub fn fetch_openrouter_models() -> Option<Vec<ModelInfo>> {
    let output = Command::new("curl")
        .args([
            "-s",
            "-m",
            &FETCH_TIMEOUT_SECS.to_string(),
            "-A",
            "herdr-pi-sidebar/0.1 github.com/mastnacek/pi-herdr-sidebar",
            "https://openrouter.ai/api/v1/models",
        ])
        .output()
        .ok()?;

    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }
    let body = String::from_utf8_lossy(&output.stdout);
    let models = parse_openrouter_json(&body);
    if !models.is_empty() {
        save_cached_models(&models);
        Some(models)
    } else {
        None
    }
}

fn cache_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("HERDR_PLUGIN_STATE_DIR") {
        let p = PathBuf::from(dir).join("openrouter_models.json");
        return Some(p);
    }
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(|h| {
            PathBuf::from(h)
                .join(".pi")
                .join("agent")
                .join("openrouter_models.json")
        })
}

pub fn save_cached_models(models: &[ModelInfo]) {
    let Some(path) = cache_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(models) {
        let _ = std::fs::write(path, json);
    }
}

pub fn load_cached_models() -> Option<Vec<ModelInfo>> {
    let path = cache_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<Vec<ModelInfo>>(&content).ok()
}

/// Filter and rank models matching a search query.
pub fn filter_models<'a>(models: &'a [ModelInfo], query: &str) -> Vec<&'a ModelInfo> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return models.iter().collect();
    }
    models
        .iter()
        .filter(|m| {
            m.id.to_lowercase().contains(&q)
                || m.name.to_lowercase().contains(&q)
                || (q == "free" && m.is_free)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openrouter_json_and_prices() {
        let sample = r#"{
            "data": [
                {
                    "id": "anthropic/claude-3.7-sonnet",
                    "name": "Anthropic: Claude 3.7 Sonnet",
                    "context_length": 200000,
                    "pricing": {
                        "prompt": "0.000003",
                        "completion": "0.000015"
                    }
                },
                {
                    "id": "meta-llama/llama-3.3-70b-instruct:free",
                    "name": "Llama 3.3 70B (free)",
                    "context_length": 131072,
                    "pricing": {
                        "prompt": "0",
                        "completion": "0"
                    }
                }
            ]
        }"#;

        let models = parse_openrouter_json(sample);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "anthropic/claude-3.7-sonnet");
        assert_eq!(models[0].context_label(), "200k");
        assert_eq!(models[0].prompt_price_m, 3.0);
        assert_eq!(models[0].completion_price_m, 15.0);
        assert_eq!(models[0].price_label(), "$3.00 / $15.00 /1M");
        assert!(!models[0].is_free);

        assert_eq!(models[1].id, "meta-llama/llama-3.3-70b-instruct:free");
        assert!(models[1].is_free);
        assert_eq!(models[1].price_label(), "FREE");
    }

    #[test]
    fn filters_by_query_or_free() {
        let models = vec![
            ModelInfo {
                id: "anthropic/claude-3.7-sonnet".to_string(),
                name: "Anthropic Claude".to_string(),
                description: String::new(),
                context_length: 200_000,
                prompt_price_m: 3.0,
                completion_price_m: 15.0,
                is_free: false,
            },
            ModelInfo {
                id: "qwen/qwen-2.5-coder-32b".to_string(),
                name: "Qwen 2.5 Coder".to_string(),
                description: String::new(),
                context_length: 32_000,
                prompt_price_m: 0.0,
                completion_price_m: 0.0,
                is_free: true,
            },
        ];

        let results = filter_models(&models, "qwen");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "qwen/qwen-2.5-coder-32b");

        let free_results = filter_models(&models, "free");
        assert_eq!(free_results.len(), 1);
        assert_eq!(free_results[0].id, "qwen/qwen-2.5-coder-32b");
    }
}
