use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::shared::{HerdrClient, PaneSnapshot, PluginContext};

pub use super::state_model::{SidebarState, Tab};
use super::state_refresh::{
    refresh_mcp, refresh_openrouter, refresh_quota, refresh_skills, refresh_spai,
};
use super::state_resolver::resolve_pane_binding;

impl SidebarState {
    pub fn new(target_snapshot: Option<PathBuf>) -> Self {
        let ctx = PluginContext::load();
        let herdr_client = HerdrClient::new();
        let explicit = target_snapshot.is_some();
        let own_pane_id = ctx.pane_id.clone();
        let target_tab_id = ctx.tab_id.clone();

        let mut state = Self {
            active_tab: Tab::Zen,
            scroll: 0,
            snapshot_path: target_snapshot,
            snapshot: None,
            last_mtime: None,
            panes: Vec::new(),
            herdr_client,
            last_refresh: SystemTime::now(),
            target_pane_id: None,
            target_tab_id,
            own_pane_id,
            explicit_snapshot: explicit,
            refresh_timer: 0,
            refresh_progress: 1.0,
            refresh_status: "Připraveno".to_string(),
            last_live: None,
            last_revision: 0,
            last_key: String::new(),
            missing_ticks: 0,
            anim_tick: 0,
            last_active_skill: None,
            live: None,
            live_session_id: None,
            live_session_mtime: None,
            skills: None,
            skills_mtime: None,
            mcp: None,
            mcp_mtime: None,
            last_mcp_calls_count: 0,
            spai: None,
            spai_mtime: None,
            quota: None,
            openrouter_credits: None,
            weather: None,
            weather_location_index: crate::slices::telemetry::weather_live::load_selected_location(
            ),
            weather_last_fetch: 0,
        };

        state.refresh(true);
        state
    }

    pub fn trigger_manual_refresh(&mut self) {
        self.refresh_timer = 10;
        self.refresh_progress = 0.1;
        self.refresh(true);
    }

    pub fn tick_animation(&mut self) {
        self.anim_tick = self.anim_tick.wrapping_add(1);
        if self.refresh_timer > 0 {
            self.refresh_timer -= 1;
            let total = if self.refresh_status.contains("obnovuje") {
                24.0
            } else {
                10.0
            };
            self.refresh_progress = 1.0 - (self.refresh_timer as f64 / total).clamp(0.0, 1.0);
            if self.refresh_timer == 0 {
                self.refresh(false);
            }
        }
    }

    pub fn set_tab(&mut self, tab: Tab) {
        if self.active_tab == tab {
            return;
        }
        self.active_tab = tab;
        self.scroll = 0;

        if let Some(path) = &self.snapshot_path {
            let tab_id = match tab {
                Tab::Status => "status",
                Tab::Skills => "skills",
                Tab::Zen | Tab::Mcp => return,
            };

            let col = if let Some(snap) = &self.snapshot {
                if let Some(hits) = &snap.tab_hits {
                    if let Some(hit) = hits
                        .iter()
                        .find(|h: &&crate::shared::snapshot::TabHit| h.id == tab_id)
                    {
                        (hit.start + hit.end) / 2 + 3
                    } else if tab_id == "status" {
                        5
                    } else {
                        15
                    }
                } else if tab_id == "status" {
                    5
                } else {
                    15
                }
            } else if tab_id == "status" {
                5
            } else {
                15
            };

            let _ = crate::shared::write_tab_request(path, col as u16, 1);
        }
    }

    pub fn next_tab(&mut self) {
        let next = (self.active_tab.to_index() + 1) % 4;
        self.set_tab(Tab::from_index(next));
    }

    pub fn prev_tab(&mut self) {
        let prev = if self.active_tab.to_index() == 0 {
            3
        } else {
            self.active_tab.to_index() - 1
        };
        self.set_tab(Tab::from_index(prev));
    }

    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll = self.scroll.saturating_sub(lines);
    }

    pub fn scroll_down(&mut self, lines: u16) {
        self.scroll = self.scroll.saturating_add(lines);
    }

    pub fn handle_mouse_click(&mut self, col: u16, row: u16) {
        if row <= 3 {
            if col < 11 {
                self.set_tab(Tab::Zen);
            } else if col < 24 {
                self.set_tab(Tab::Status);
            } else if col < 37 {
                self.set_tab(Tab::Skills);
            } else if col < 50 {
                self.set_tab(Tab::Mcp);
            }
        }
    }

    pub fn refresh(&mut self, force: bool) {
        self.last_refresh = SystemTime::now();
        self.panes = self.herdr_client.list_panes();

        if !self.explicit_snapshot {
            let current_tab = self.target_tab_id.as_deref();
            let own_pane = self.own_pane_id.as_deref().unwrap_or("");
            let binding = resolve_pane_binding(&self.panes, current_tab, own_pane);
            self.target_pane_id = binding.target_pane_id;
            self.snapshot_path = binding.snapshot_path;
            self.live_session_id = binding.live_session_id;
            self.refresh_status = binding.refresh_status;
        }

        self.refresh_live(force);
        refresh_skills(
            self.live.as_ref(),
            self.snapshot_path.as_deref(),
            &mut self.skills_mtime,
            &mut self.skills,
            force,
        );
        refresh_mcp(
            self.live.as_ref(),
            &mut self.mcp_mtime,
            &mut self.mcp,
            force,
        );

        let cwd_str = self.live.as_ref().map(|l| l.cwd.as_str()).or_else(|| {
            self.panes
                .iter()
                .find(|p| self.target_pane_id.as_deref() == Some(p.pane_id.as_str()))
                .and_then(|p| p.cwd.as_deref())
        });
        refresh_spai(cwd_str, &mut self.spai_mtime, &mut self.spai, force);

        self.quota = Some(refresh_quota(self.live.as_ref()));

        let pane_cwd = self
            .panes
            .iter()
            .find(|p| self.target_pane_id.as_deref() == Some(p.pane_id.as_str()))
            .and_then(|p| p.cwd.as_deref())
            .map(Path::new);
        self.openrouter_credits = refresh_openrouter(pane_cwd, force);

        self.refresh_weather(force);

        let (should_switch_mcp, new_total_calls) = if let Some(mcp) = &self.mcp {
            let active = mcp.in_flight || mcp.total_calls > self.last_mcp_calls_count;
            let switch = active && self.last_mcp_calls_count > 0 && self.active_tab != Tab::Mcp;
            (switch, mcp.total_calls)
        } else {
            (false, 0)
        };
        if new_total_calls > 0 {
            self.last_mcp_calls_count = new_total_calls;
        }
        if should_switch_mcp {
            self.set_tab(Tab::Mcp);
        }

        if let Some(path) = &self.snapshot_path {
            if let Ok(meta) = std::fs::metadata(path) {
                if let Ok(mtime) = meta.modified() {
                    if force || self.last_mtime != Some(mtime) {
                        self.last_mtime = Some(mtime);
                        if let Some(snap) = PaneSnapshot::read_from_file(path) {
                            self.ingest_snapshot(snap);
                        }
                    }
                    self.missing_ticks = 0;
                }
            } else {
                self.missing_ticks = self.missing_ticks.saturating_add(1);
                if self.missing_ticks >= 3 {
                    if self.snapshot.is_some() {
                        self.refresh_timer = 10;
                        self.refresh_progress = 0.0;
                    }
                    self.snapshot = None;
                    self.last_mtime = None;
                    self.last_live = None;
                    self.refresh_status = "Snapshot smazán (reload?)".to_string();
                }
            }
        }
    }

    fn refresh_weather(&mut self, force: bool) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if !force && now.saturating_sub(self.weather_last_fetch) < 60 {
            return;
        }
        self.weather_last_fetch = now;
        self.weather = Some(crate::slices::telemetry::weather_live::refresh_weather(
            self.weather_location_index,
            force,
        ));
    }

    pub fn cycle_weather_location(&mut self) {
        self.weather_location_index = (self.weather_location_index + 1)
            % crate::slices::telemetry::weather_live::LOCATIONS.len();
        crate::slices::telemetry::weather_live::save_selected_location(self.weather_location_index);
        self.refresh_weather(true);
    }

    fn ingest_snapshot(&mut self, snap: PaneSnapshot) {
        let prev_live = self.last_live;
        let prev_rev = self.last_revision;
        let prev_key = self.last_key.clone();

        let went_dead = prev_live == Some(true) && !snap.live;
        let came_back = prev_live == Some(false) && snap.live;
        let restarted = prev_rev > 0
            && (snap.revision < prev_rev
                || (snap.live && !prev_key.is_empty() && snap.key != prev_key));

        if went_dead {
            self.refresh_timer = 24;
            self.refresh_progress = 0.0;
            self.refresh_status = "Pi relace se obnovuje…".to_string();
        } else if came_back || restarted {
            self.refresh_timer = 12;
            self.refresh_progress = 0.0;
            self.refresh_status = "Relace obnovena — načteno znovu".to_string();
            self.scroll = 0;
        }

        self.last_live = Some(snap.live);
        self.last_revision = snap.revision;
        self.last_key = snap.key.clone();
        self.snapshot = Some(snap);
    }

    fn refresh_live(&mut self, force: bool) {
        let herdr_session: Option<String> = self
            .live_session_id
            .as_ref()
            .filter(|s| s.len() >= 8)
            .cloned();
        let pane_cwd = self
            .panes
            .iter()
            .find(|p| self.target_pane_id.as_deref() == Some(p.pane_id.as_str()))
            .and_then(|p| p.cwd.clone());

        if let Some(session_id) = herdr_session {
            let prefix = session_id.get(..8).unwrap_or(&session_id).to_string();
            let cwd = pane_cwd.clone().unwrap_or_default();
            let file = crate::slices::telemetry::find_session_file(&prefix, &cwd)
                .or_else(|| crate::slices::telemetry::find_session_file(&session_id, &cwd))
                .or_else(|| crate::slices::telemetry::find_newest_session(pane_cwd.as_deref()));
            self.load_live_from(file, &session_id, force);
        } else {
            let file = crate::slices::telemetry::find_newest_session_scoped(pane_cwd.as_deref());
            self.load_live_from(file, "", force);
        }
    }

    fn load_live_from(&mut self, file: Option<std::path::PathBuf>, session_id: &str, force: bool) {
        let Some(file) = file else {
            self.live = None;
            return;
        };

        let Ok(meta) = std::fs::metadata(&file) else {
            self.live = None;
            return;
        };
        let mtime = meta.modified().ok();
        if !force && self.live.is_some() && mtime == self.live_session_mtime {
            return;
        }
        self.live_session_mtime = mtime;
        if let Some(t) = crate::slices::telemetry::parse_session(&file, session_id) {
            self.live = Some(t);
        }
    }
}
