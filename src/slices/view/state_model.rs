use std::path::PathBuf;
use std::time::SystemTime;

use crate::shared::{HerdrClient, HerdrPaneInfo, PaneSnapshot};
use crate::slices::telemetry::{
    mcp_live::McpTelemetry, openrouter_live::OpenRouterCreditTelemetry,
    prompt_sidecar::PromptSidecar, quota_live::QuotaTelemetry, skills::SkillSnapshotFile,
    spai_live::SpaiTelemetry, weather_live::WeatherTelemetry, LiveTelemetry,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Zen = 0,
    Status = 1,
    Skills = 2,
    Mcp = 3,
    Notes = 4,
    Shortcuts = 5,
    Settings = 6,
}

/// Number of tabs; keep in sync with [`Tab`] and [`TAB_LABELS`].
pub const TAB_COUNT: usize = 7;

/// Tab bar labels, in index order. Single source of truth so the header and the
/// click hit-testing cannot drift apart.
pub const TAB_LABELS: [&str; TAB_COUNT] = [
    " 0: Zen ",
    " 1: Status ",
    " 2: Skills ",
    " 3: MCP ",
    " 4: Notes ",
    " 5: Shortcuts ",
    " 6: Settings ",
];

/// Maps a mouse click column to a tab index by mirroring the exact geometry
/// the `Tabs` widget (ratatui 0.29) uses when rendering into the header block:
///
/// - the block is bordered, so tabs start at column 1 (past the left border);
/// - each tab cell is `padding_left` (1 space) + title + `padding_right` (1 space);
/// - the divider (1 column) is drawn *between* tabs, not after the last one.
///
/// Any divergence from this math silently mis-routes clicks (or drops them) —
/// keep it in lockstep with `render_header` in `view/ui.rs`.
pub fn tab_index_at(col: u16) -> Option<usize> {
    let mut x = 1u16; // left border of the header block
    for (index, label) in TAB_LABELS.iter().enumerate() {
        let width = label.chars().count() as u16 + 2; // padding left + right
        if col >= x && col < x.saturating_add(width) {
            return Some(index);
        }
        x += width + 1; // divider between tabs
    }
    None
}

impl Tab {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Tab::Zen,
            2 => Tab::Skills,
            3 => Tab::Mcp,
            4 => Tab::Notes,
            5 => Tab::Shortcuts,
            6 => Tab::Settings,
            _ => Tab::Status,
        }
    }

    pub fn to_index(self) -> usize {
        self as usize
    }
}

pub struct SidebarState {
    pub active_tab: Tab,
    /// Plugin state directory from the injected environment, when herdr set it.
    /// `tab_state` falls back to the pane-state directory without it.
    pub state_dir: Option<PathBuf>,
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
    /// Exact prompt provenance captured in `before_agent_start` (TS extension).
    pub prompt_sidecar: Option<PromptSidecar>,
    pub prompt_sidecar_mtime: Option<SystemTime>,
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
    pub spai_notes: crate::slices::spai_notes::SpaiNotesState,
    pub shortcuts: crate::slices::shortcuts::ShortcutsState,
    pub settings: crate::slices::settings::SettingsState,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Width of one rendered tab cell (padding + label), mirroring `tab_index_at`.
    fn cell_width(label: &str) -> u16 {
        label.chars().count() as u16 + 2
    }

    #[test]
    fn click_on_each_tab_center_selects_that_tab() {
        let mut x = 1u16; // left border
        for (index, label) in TAB_LABELS.iter().enumerate() {
            let center = x + cell_width(label) / 2;
            assert_eq!(
                tab_index_at(center),
                Some(index),
                "click at col {center} should hit tab {index} ({label})"
            );
            x += cell_width(label) + 1; // divider
        }
    }

    #[test]
    fn click_on_borders_and_gaps_does_not_crash_or_hit_wrong_tab() {
        // Left border of the block: no tab there.
        assert_eq!(tab_index_at(0), None);
        // Divider column right after the last tab: strip is over, nothing to hit.
        let mut x = 1u16;
        for label in TAB_LABELS.iter() {
            x += cell_width(label) + 1;
        }
        assert_eq!(tab_index_at(x), None);
        // A column far past the strip.
        assert_eq!(tab_index_at(500), None);
    }

    #[test]
    fn first_and_last_boundaries_map_to_first_and_last_tab() {
        // First cell starts at col 1 (past border).
        assert_eq!(tab_index_at(1), Some(0));
        let last = TAB_LABELS.len() - 1;
        let mut x = 1u16;
        for (i, label) in TAB_LABELS.iter().enumerate() {
            if i == last {
                break;
            }
            x += cell_width(label) + 1;
        }
        assert_eq!(tab_index_at(x), Some(last));
    }
}
