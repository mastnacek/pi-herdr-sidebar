//! Dynamic OpenRouter models catalog, pricing, and search.
use serde::{Deserialize, Serialize};

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
    pub is_embedding: bool,
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
struct OpenRouterArchitectureRaw {
    #[serde(default)]
    modality: Option<String>,
    #[serde(default)]
    output_modalities: Option<Vec<String>>,
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
    #[serde(default)]
    architecture: Option<OpenRouterArchitectureRaw>,
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

            let is_embedding = m.id.to_lowercase().contains("embed")
                || name.to_lowercase().contains("embed")
                || m.architecture
                    .as_ref()
                    .and_then(|a| a.modality.as_ref())
                    .map(|modality| modality.contains("embedding"))
                    .unwrap_or(false)
                || m.architecture
                    .as_ref()
                    .and_then(|a| a.output_modalities.as_ref())
                    .map(|outputs| outputs.iter().any(|o| o.contains("embedding")))
                    .unwrap_or(false);

            ModelInfo {
                id: m.id,
                name,
                description,
                context_length,
                prompt_price_m: prompt_m,
                completion_price_m: completion_m,
                is_free,
                is_embedding,
            }
        })
        .collect()
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
                || (q == "embed" && m.is_embedding)
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
                    "id": "qwen/qwen3-embedding-8b",
                    "name": "Qwen: Qwen3 Embedding 8B",
                    "context_length": 32768,
                    "architecture": {
                        "modality": "text->embeddings",
                        "output_modalities": ["embeddings"]
                    },
                    "pricing": {
                        "prompt": "0.00000001",
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
        assert!(!models[0].is_embedding);

        assert_eq!(models[1].id, "qwen/qwen3-embedding-8b");
        assert!(models[1].is_embedding);
        assert_eq!(models[1].context_label(), "32k");
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
                is_embedding: false,
            },
            ModelInfo {
                id: "qwen/qwen3-embedding-8b".to_string(),
                name: "Qwen3 Embedding 8B".to_string(),
                description: String::new(),
                context_length: 32_768,
                prompt_price_m: 0.01,
                completion_price_m: 0.0,
                is_free: false,
                is_embedding: true,
            },
        ];

        let results = filter_models(&models, "qwen");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "qwen/qwen3-embedding-8b");

        let embed_results = filter_models(&models, "embed");
        assert_eq!(embed_results.len(), 1);
        assert_eq!(embed_results[0].id, "qwen/qwen3-embedding-8b");
    }
}
