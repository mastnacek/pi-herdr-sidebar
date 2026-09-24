use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct OpenRouterAccountCredit {
    pub id: String,
    pub label: String,
    pub total_credits: f64,
    pub total_usage: f64,
    pub remaining_credits: f64,
}

#[derive(Debug, Clone, Default)]
pub struct OpenRouterCreditTelemetry {
    pub accounts: Vec<OpenRouterAccountCredit>,
}

#[derive(Deserialize)]
struct AccountConfig {
    id: String,
    label: Option<String>,
    #[serde(rename = "apiKey")]
    api_key: Option<String>,
}

#[derive(Deserialize)]
struct AccountsConfigFile {
    #[serde(default)]
    accounts: Vec<AccountConfig>,
}

#[derive(Deserialize)]
struct CredentialEntry {
    #[serde(rename = "type")]
    _cred_type: Option<String>,
    key: Option<String>,
    access: Option<String>,
}

#[derive(Deserialize)]
struct OpenRouterApiResponse {
    data: Option<OpenRouterApiData>,
}

#[derive(Deserialize)]
struct OpenRouterApiData {
    total_credits: Option<f64>,
    total_usage: Option<f64>,
}

static OPENROUTER_CACHE: Mutex<Option<(Instant, OpenRouterCreditTelemetry)>> = Mutex::new(None);
const OR_CACHE_TTL: Duration = Duration::from_secs(60);

pub fn is_openrouter_provider(provider: &str) -> bool {
    let p = provider.trim().to_lowercase();
    p == "openrouter" || p.starts_with("openrouter-") || p.contains("openrouter")
}

fn agent_dir() -> Option<PathBuf> {
    if let Ok(override_dir) = std::env::var("PI_CODING_AGENT_DIR") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    let home = crate::shared::dirs_home()?;
    Some(home.join(".pi").join("agent"))
}

fn resolve_config_path(cwd: Option<&Path>) -> Option<PathBuf> {
    if let Ok(from_env) = std::env::var("PI_OPENROUTER_ACCOUNTS") {
        let p = PathBuf::from(from_env.trim());
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(c) = cwd {
        let project = c.join(".pi").join("openrouter-accounts.json");
        if project.exists() {
            return Some(project);
        }
    }
    let global = agent_dir()?.join("openrouter-accounts.json");
    if global.exists() {
        return Some(global);
    }
    None
}

fn read_auth() -> HashMap<String, CredentialEntry> {
    let Some(dir) = agent_dir() else {
        return HashMap::new();
    };
    let auth_path = dir.join("auth.json");
    let Ok(content) = fs::read_to_string(auth_path) else {
        return HashMap::new();
    };
    serde_json::from_str::<HashMap<String, CredentialEntry>>(&content).unwrap_or_default()
}

fn provider_id_for(account_id: &str) -> String {
    let slug: String = account_id
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() || slug == "openrouter" {
        "openrouter".to_string()
    } else {
        format!("openrouter-{}", slug)
    }
}

fn resolve_key_source(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(var) = s.strip_prefix('$') {
        let var_name = var.trim_matches(|c| c == '{' || c == '}');
        return std::env::var(var_name)
            .ok()
            .filter(|v| !v.trim().is_empty());
    }
    if let Some(cmd) = s.strip_prefix('!') {
        let output = if cfg!(windows) {
            Command::new("cmd").args(["/C", cmd.trim()]).output().ok()?
        } else {
            Command::new("sh").args(["-c", cmd.trim()]).output().ok()?
        };
        if output.status.success() {
            let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !key.is_empty() {
                return Some(key);
            }
        }
        return None;
    }
    Some(s.to_string())
}

fn resolve_account_key(
    account: &AccountConfig,
    provider_id: &str,
    auth: &HashMap<String, CredentialEntry>,
) -> Option<String> {
    if let Some(entry) = auth.get(provider_id) {
        if let Some(k) = entry.key.as_deref().or(entry.access.as_deref()) {
            let trimmed = k.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    if let Some(raw) = &account.api_key {
        if let Some(k) = resolve_key_source(raw) {
            return Some(k);
        }
    }
    let env_var_name = format!(
        "OPENROUTER_{}_API_KEY",
        account
            .id
            .to_uppercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>()
    );
    if let Ok(v) = std::env::var(&env_var_name) {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if account.id == "default" || provider_id == "openrouter" {
        if let Some(entry) = auth.get("openrouter") {
            if let Some(k) = entry.key.as_deref().or(entry.access.as_deref()) {
                let trimmed = k.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        if let Ok(v) = std::env::var("OPENROUTER_API_KEY") {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn query_openrouter_credits(key: &str) -> Option<(f64, f64)> {
    let output = Command::new("curl")
        .args([
            "-s",
            "-m",
            "5",
            "-H",
            &format!("Authorization: Bearer {}", key),
            "https://openrouter.ai/api/v1/credits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let parsed: OpenRouterApiResponse = serde_json::from_slice(&output.stdout).ok()?;
    let data = parsed.data?;
    Some((
        data.total_credits.unwrap_or(0.0),
        data.total_usage.unwrap_or(0.0),
    ))
}

pub fn fetch_openrouter_credits(cwd: Option<&Path>, force: bool) -> OpenRouterCreditTelemetry {
    if !force {
        if let Ok(guard) = OPENROUTER_CACHE.lock() {
            if let Some((instant, telemetry)) = guard.as_ref() {
                if instant.elapsed() < OR_CACHE_TTL {
                    return telemetry.clone();
                }
            }
        }
    }

    let auth = read_auth();
    let config_path = resolve_config_path(cwd);
    let mut accounts_to_check: Vec<(String, String, Option<String>)> = Vec::new();

    if let Some(cfg_path) = config_path {
        if let Ok(content) = fs::read_to_string(cfg_path) {
            if let Ok(cfg) = serde_json::from_str::<AccountsConfigFile>(&content) {
                for acc in cfg.accounts {
                    let pid = provider_id_for(&acc.id);
                    let label = acc.label.clone().unwrap_or_else(|| acc.id.clone());
                    let key = resolve_account_key(&acc, &pid, &auth);
                    accounts_to_check.push((acc.id, label, key));
                }
            }
        }
    }

    let has_root = accounts_to_check
        .iter()
        .any(|(id, _, _)| id == "default" || id == "openrouter");
    if !has_root {
        let root_key = auth
            .get("openrouter")
            .and_then(|e| e.key.as_deref().or(e.access.as_deref()))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                std::env::var("OPENROUTER_API_KEY")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            });
        if let Some(k) = root_key {
            accounts_to_check.push(("openrouter".to_string(), "OpenRouter".to_string(), Some(k)));
        }
    }

    for (k, entry) in &auth {
        if let Some(slug) = k.strip_prefix("openrouter-") {
            if !accounts_to_check.iter().any(|(id, _, _)| id == slug) {
                if let Some(key) = entry.key.as_deref().or(entry.access.as_deref()) {
                    let trimmed = key.trim();
                    if !trimmed.is_empty() {
                        accounts_to_check.push((
                            slug.to_string(),
                            slug.to_string(),
                            Some(trimmed.to_string()),
                        ));
                    }
                }
            }
        }
    }

    let mut results = Vec::new();
    for (id, label, key_opt) in accounts_to_check {
        let Some(key) = key_opt else {
            continue;
        };

        if let Some((credits, usage)) = query_openrouter_credits(&key) {
            let remaining = credits - usage;
            results.push(OpenRouterAccountCredit {
                id,
                label,
                total_credits: credits,
                total_usage: usage,
                remaining_credits: remaining,
            });
        }
    }

    let telemetry = OpenRouterCreditTelemetry { accounts: results };
    if let Ok(mut guard) = OPENROUTER_CACHE.lock() {
        *guard = Some((Instant::now(), telemetry.clone()));
    }
    telemetry
}
