//! Which Claude Code plan the pane runs on, for the banner's provider slot
//! (`(Claude Code · Max 5x) claude-opus-5-5` instead of a bare `anthropic`).
//!
//! Source: `oauthAccount` in `~/.claude.json` — the account profile Claude
//! Code caches after login. It holds no secrets (tokens live in
//! `~/.claude/.credentials.json`, which this module never reads). An API-key
//! login has no `oauthAccount`, so it reads as `API`.

use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Deserialize, Default)]
struct ClaudeJson {
    #[serde(default, rename = "oauthAccount")]
    oauth_account: Option<OauthAccount>,
}

#[derive(Deserialize, Default)]
struct OauthAccount {
    #[serde(default, rename = "organizationType")]
    organization_type: Option<String>,
    #[serde(default, rename = "organizationRateLimitTier")]
    rate_limit_tier: Option<String>,
}

/// `Claude Code · <plan>`; read once per process (the file is large and the
/// plan changes only with a new login).
pub fn label() -> &'static str {
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(|| {
        let text = crate::shared::dirs_home()
            .and_then(|h| std::fs::read_to_string(h.join(".claude.json")).ok())
            .unwrap_or_default();
        label_from(&text)
    })
}

pub(super) fn label_from(claude_json: &str) -> String {
    let account = serde_json::from_str::<ClaudeJson>(claude_json)
        .ok()
        .and_then(|j| j.oauth_account);
    let plan = match account {
        None => "API".to_string(),
        Some(a) => plan_name(
            a.organization_type.as_deref().unwrap_or(""),
            a.rate_limit_tier.as_deref().unwrap_or(""),
        ),
    };
    format!("Claude Code · {plan}")
}

/// `claude_max` + `default_claude_max_5x` → `Max 5x`; unknown types are shown
/// as written, without the `claude_` prefix.
fn plan_name(org_type: &str, tier: &str) -> String {
    let base = match org_type {
        "claude_max" => "Max",
        "claude_pro" => "Pro",
        "claude_team" => "Team",
        "claude_enterprise" => "Enterprise",
        "" => return "předplatné".to_string(),
        other => other.strip_prefix("claude_").unwrap_or(other),
    };
    match tier.rsplit('_').next() {
        Some(mult) if mult.ends_with('x') && mult[..mult.len() - 1].parse::<u32>().is_ok() => {
            format!("{base} {mult}")
        }
        _ => base.to_string(),
    }
}
