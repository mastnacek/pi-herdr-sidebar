use std::path::Path;
use std::time::SystemTime;

use crate::slices::telemetry::prompt_sidecar::{sidecar_path, PromptSidecar};
use crate::slices::telemetry::weather_live::WeatherTelemetry;
use crate::slices::telemetry::{
    mcp_live::McpTelemetry, openrouter_live::OpenRouterCreditTelemetry, quota_live::QuotaTelemetry,
    skills::SkillSnapshotFile, spai_live::SpaiTelemetry, LiveTelemetry,
};

pub fn refresh_skills(
    live: Option<&LiveTelemetry>,
    snapshot_path: Option<&Path>,
    skills_mtime: &mut Option<SystemTime>,
    skills: &mut Option<SkillSnapshotFile>,
    force: bool,
) {
    if let Some(t) = live {
        if let Some(f) = &t.session_file {
            if let Ok(meta) = std::fs::metadata(f) {
                let mtime = meta.modified().ok();
                if force || *skills_mtime != mtime || skills.is_none() {
                    *skills_mtime = mtime;
                    *skills = crate::slices::telemetry::skills_live::parse_session_skills(f);
                }
                return;
            }
        }
    }

    let Some(path) = snapshot_path else {
        *skills = None;
        return;
    };
    let mut skills_path = path.to_path_buf();
    let name = skills_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sidebar")
        .to_string();
    let base = name.strip_suffix(".json").unwrap_or(&name).to_string();
    skills_path.set_file_name(format!("{}.skills.json", base));

    let Ok(meta) = std::fs::metadata(&skills_path) else {
        *skills = None;
        return;
    };
    let mtime = meta.modified().ok();
    if !force && skills.is_some() && *skills_mtime == mtime {
        return;
    }
    *skills_mtime = mtime;
    *skills = crate::slices::telemetry::skills::SkillSnapshotFile::read_from_file(&skills_path);
}

pub fn refresh_mcp(
    live: Option<&LiveTelemetry>,
    mcp_mtime: &mut Option<SystemTime>,
    mcp: &mut Option<McpTelemetry>,
    force: bool,
) {
    if let Some(t) = live {
        if let Some(f) = &t.session_file {
            if let Ok(meta) = std::fs::metadata(f) {
                let mtime = meta.modified().ok();
                if force || *mcp_mtime != mtime || mcp.is_none() {
                    *mcp_mtime = mtime;
                    *mcp = crate::slices::telemetry::mcp_live::parse_mcp_session(f);
                }
                return;
            }
        }
    }
    *mcp = None;
}

pub fn refresh_prompt_sidecar(
    snapshot_path: Option<&Path>,
    mtime: &mut Option<SystemTime>,
    sidecar: &mut Option<PromptSidecar>,
    force: bool,
) {
    let Some(path) = snapshot_path.map(sidecar_path) else {
        *sidecar = None;
        return;
    };
    let Ok(meta) = std::fs::metadata(&path) else {
        *sidecar = None;
        return;
    };
    let modified = meta.modified().ok();
    if !force && sidecar.is_some() && *mtime == modified {
        return;
    }
    *mtime = modified;
    *sidecar = PromptSidecar::read_from_file(&path);
}

pub fn refresh_spai(
    cwd: Option<&str>,
    spai_mtime: &mut Option<SystemTime>,
    spai: &mut Option<SpaiTelemetry>,
    force: bool,
) {
    let index_path = crate::slices::telemetry::spai_live::find_spai_index_path(cwd);
    let Some(path) = index_path else {
        *spai = None;
        return;
    };
    let Ok(meta) = std::fs::metadata(&path) else {
        *spai = None;
        return;
    };
    let modified = meta.modified().ok();
    if !force && spai.is_some() && *spai_mtime == modified {
        return;
    }
    *spai_mtime = modified;
    *spai = crate::slices::telemetry::spai_live::parse_spai_index(&path);
}

pub fn refresh_quota(live: Option<&LiveTelemetry>) -> QuotaTelemetry {
    let mut q = QuotaTelemetry::default();
    q.antigravity = crate::slices::telemetry::quota_live::fetch_antigravity_live_quota();
    if let Some(t) = live {
        if let Some(f) = &t.session_file {
            let (w5m, w1h, w5h) =
                crate::slices::telemetry::quota_live::compute_session_sliding_windows(f);
            q.session_sliding_5m_tokens = w5m;
            q.session_sliding_1h_tokens = w1h;
            q.session_sliding_5h_tokens = w5h;
        }
    }
    q
}

pub fn refresh_openrouter(
    pane_cwd: Option<&Path>,
    force: bool,
) -> Option<OpenRouterCreditTelemetry> {
    Some(crate::slices::telemetry::openrouter_live::fetch_openrouter_credits(pane_cwd, force))
}

pub fn refresh_weather_telemetry(
    location_index: usize,
    last_fetch: &mut u64,
    force: bool,
) -> Option<WeatherTelemetry> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if !force && now.saturating_sub(*last_fetch) < 60 {
        return None;
    }
    *last_fetch = now;
    Some(crate::slices::telemetry::weather_live::refresh_weather(
        location_index,
        force,
    ))
}

pub fn load_live_session(
    file: Option<&Path>,
    session_id: &str,
    live_session_mtime: &mut Option<SystemTime>,
    current_live: &Option<LiveTelemetry>,
    force: bool,
) -> Option<LiveTelemetry> {
    let file = file?;
    let meta = std::fs::metadata(file).ok()?;
    let mtime = meta.modified().ok();
    if !force && current_live.is_some() && *live_session_mtime == mtime {
        return current_live.clone();
    }
    *live_session_mtime = mtime;
    crate::slices::telemetry::parse_session(file, session_id)
}
