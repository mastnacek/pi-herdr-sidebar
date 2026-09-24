use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use super::Usage;

/// Pi model catalog entry (from ~/.pi/agent/models.json overrides and
/// models-store.json — the models.dev snapshot pi maintains). Read directly
/// like the pi agent framework does; nothing hardcoded.
#[derive(Deserialize, Clone)]
pub struct ModelCost {
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(rename = "cacheRead", default)]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite", default)]
    pub cache_write: f64,
    /// Tiered pricing: full rates once prompt tokens exceed a threshold.
    #[serde(default)]
    pub tiers: Vec<CostTier>,
}

#[derive(Deserialize, Clone)]
pub struct CostTier {
    #[serde(rename = "inputTokensAbove", default)]
    pub input_tokens_above: u64,
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(rename = "cacheRead", default)]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite", default)]
    pub cache_write: f64,
}

/// USD rates per million tokens.
pub struct Rates {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

impl ModelCost {
    pub fn rates(&self) -> Rates {
        Rates {
            input: self.input,
            output: self.output,
            cache_read: self.cache_read,
            cache_write: self.cache_write,
        }
    }

    /// Byte-for-byte port of pi-ai `calculateCost` (see
    /// pi-coding-agent dist chunk: `function calculateCost(model,usage)`).
    /// Long-lived (1h) cache writes bill at 2× the input rate.
    pub fn compute_total(&self, u: &Usage) -> f64 {
        self.select_for(u).total_cost(u)
    }

    /// Select tiered rates by total prompt tokens (input + cacheRead +
    /// cacheWrite), matching pi-ai's threshold walk.
    pub fn select_for(&self, u: &Usage) -> Rates {
        let input_tokens = u.input + u.cache_read + u.cache_write;
        let mut rates = self.rates();
        let mut matched: Option<u64> = None;
        for tier in &self.tiers {
            if input_tokens > tier.input_tokens_above
                && tier.input_tokens_above > matched.unwrap_or(0)
            {
                rates = tier.rates();
                matched = Some(tier.input_tokens_above);
            }
        }
        rates
    }
}

impl CostTier {
    pub fn rates(&self) -> Rates {
        Rates {
            input: self.input,
            output: self.output,
            cache_read: self.cache_read,
            cache_write: self.cache_write,
        }
    }
}

impl Rates {
    /// USD for one usage entry, per pi-ai `calculateCost` semantics.
    pub fn total_cost(&self, u: &Usage) -> f64 {
        let long_write = u.cache_write_1h.min(u.cache_write);
        let short_write = u.cache_write - long_write;
        let cache_write_cost =
            self.cache_write * short_write as f64 + self.input * 2.0 * long_write as f64;
        (self.input * u.input as f64
            + self.output * u.output as f64
            + self.cache_read * u.cache_read as f64
            + cache_write_cost)
            / 1e6
    }
}

/// One model row from the pi catalog.
#[derive(Deserialize, Clone)]
pub struct ModelEntry {
    pub id: String,
    #[serde(default, rename = "contextWindow")]
    pub context_window: u64,
    #[serde(default)]
    pub cost: Option<ModelCost>,
}

#[derive(Deserialize)]
struct ProviderCfg {
    #[serde(default)]
    models: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ProvidersFile {
    #[serde(default)]
    providers: std::collections::BTreeMap<String, ProviderCfg>,
}

#[derive(Deserialize)]
struct TopLevelFile {
    #[serde(flatten)]
    providers: std::collections::BTreeMap<String, ProviderCfg>,
}

pub type Catalog = Vec<(String, Vec<ModelEntry>)>;

static CATALOG: Mutex<Option<(Option<SystemTime>, Option<SystemTime>, Catalog)>> = Mutex::new(None);

fn catalog_mtimes() -> (Option<SystemTime>, Option<SystemTime>) {
    let home = match crate::shared::dirs_home() {
        Some(h) => h,
        None => return (None, None),
    };
    let mtime = |p: PathBuf| fs::metadata(p).and_then(|m| m.modified()).ok();
    (
        mtime(home.join(".pi").join("agent").join("models.json")),
        mtime(home.join(".pi").join("agent").join("models-store.json")),
    )
}

pub fn load_catalog() -> Catalog {
    let (m1, m2) = catalog_mtimes();
    if let Ok(guard) = CATALOG.lock() {
        if let Some((c1, c2, cat)) = guard.as_ref() {
            if *c1 == m1 && *c2 == m2 {
                return cat.clone();
            }
        }
    }
    let mut cat: Catalog = Vec::new();
    let home = crate::shared::dirs_home();
    if let Some(home) = home {
        for (path, providers_key) in [
            (home.join(".pi").join("agent").join("models.json"), true),
            (
                home.join(".pi").join("agent").join("models-store.json"),
                false,
            ),
        ] {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let providers = if providers_key {
                serde_json::from_str::<ProvidersFile>(&text).map(|f| f.providers)
            } else {
                serde_json::from_str::<TopLevelFile>(&text).map(|f| f.providers)
            };
            if let Ok(providers) = providers {
                for (name, cfg) in providers {
                    cat.push((name, cfg.models));
                }
            }
        }
    }
    if let Ok(mut guard) = CATALOG.lock() {
        *guard = Some((m1, m2, cat.clone()));
    }
    cat
}

pub fn catalog_lookup(provider: &str, model_id: &str) -> Option<ModelEntry> {
    let cat = load_catalog();
    let mut any_match: Option<&ModelEntry> = None;
    for (pname, models) in &cat {
        if !provider.is_empty() && pname != provider {
            continue;
        }
        if let Some(m) = models.iter().find(|m| m.id == model_id) {
            if pname == provider {
                return Some(m.clone());
            }
            any_match = any_match.or(Some(m));
        }
    }
    any_match.cloned()
}

pub fn context_window_for(provider: &str, model_id: &str) -> u64 {
    catalog_lookup(provider, model_id)
        .map(|m| m.context_window)
        .unwrap_or(0)
}

pub fn catalog_cost(provider: &str, model_id: &str) -> Option<ModelCost> {
    catalog_lookup(provider, model_id).and_then(|m| m.cost)
}
