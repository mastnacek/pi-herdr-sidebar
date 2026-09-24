use std::path::PathBuf;
use std::time::SystemTime;

use crate::shared::{HerdrClient, HerdrPaneInfo, PaneSnapshot};
use crate::slices::telemetry::{
    mcp_live::McpTelemetry, openrouter_live::OpenRouterCreditTelemetry, quota_live::QuotaTelemetry,
    skills::SkillSnapshotFile, spai_live::SpaiTelemetry, weather_live::WeatherTelemetry,
    LiveTelemetry,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Zen = 0,
    Status = 1,
    Skills = 2,
    Mcp = 3,
}

impl Tab {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Tab::Zen,
            2 => Tab::Skills,
            3 => Tab::Mcp,
            _ => Tab::Status,
        }
    }

    pub fn to_index(self) -> usize {
        self as usize
    }
}

pub struct SidebarState {
    pub active_tab: Tab,
    pub scroll: u16,
    pub snapshot_path: Option<PathBuf>,
    pub snapshot: Option<PaneSnapshot>,
    pub last_mtime: Option<SystemTime>,
    pub panes: Vec<HerdrPaneInfo>,
    pub herdr_client: HerdrClient,
    pub last_refresh: SystemTime,
    pub target_pane_id: Option<String>,
    pub target_tab_id: Option<String>,
    pub own_pane_id: Option<String>,
    pub explicit_snapshot: bool,
    pub refresh_timer: u8,
    pub refresh_progress: f64,
    pub refresh_status: String,
    pub last_live: Option<bool>,
    pub last_revision: u64,
    pub last_key: String,
    pub missing_ticks: u8,
    pub anim_tick: u64,
    pub last_active_skill: Option<String>,
    pub live: Option<LiveTelemetry>,
    pub live_session_id: Option<String>,
    pub live_session_mtime: Option<SystemTime>,
    pub skills: Option<SkillSnapshotFile>,
    pub skills_mtime: Option<SystemTime>,
    pub mcp: Option<McpTelemetry>,
    pub mcp_mtime: Option<SystemTime>,
    pub last_mcp_calls_count: u64,
    pub spai: Option<SpaiTelemetry>,
    pub spai_mtime: Option<SystemTime>,
    pub quota: Option<QuotaTelemetry>,
    pub openrouter_credits: Option<OpenRouterCreditTelemetry>,
    pub weather: Option<WeatherTelemetry>,
    pub weather_location_index: usize,
    pub weather_last_fetch: u64,
}
