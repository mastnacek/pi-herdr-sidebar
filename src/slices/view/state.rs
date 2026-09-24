use crate::shared::{
    snapshot_path_for_pane, HerdrClient, HerdrPaneInfo, PaneSnapshot, PluginContext,
};
use std::path::PathBuf;
use std::time::SystemTime;

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
    /// Last detected active skill name to trigger auto-switch on skill activation
    pub last_active_skill: Option<String>,
    /// Live telemetry parsed directly from the Pi session JSONL (no TS ext needed).
    pub live: Option<crate::slices::telemetry::LiveTelemetry>,
    pub live_session_id: Option<String>,
    pub live_session_mtime: Option<SystemTime>,
    /// Structured skill state (Gates, Focus, Guidance). Primary source: parsed
    /// directly from the Pi session JSONL (`skills_live`), like the Status face.
    /// Fallback: TS sidecar (`<pane>.skills.json`) when no session file exists.
    pub skills: Option<crate::slices::telemetry::skills::SkillSnapshotFile>,
    pub skills_mtime: Option<SystemTime>,
    /// Live MCP server telemetry parsed from the Pi session JSONL.
    pub mcp: Option<crate::slices::telemetry::mcp_live::McpTelemetry>,
    pub mcp_mtime: Option<SystemTime>,
    pub last_mcp_calls_count: u64,
    /// Live SPAI task ledger parsed directly from `docs/spai/.index.json`.
    pub spai: Option<crate::slices::telemetry::spai_live::SpaiTelemetry>,
    pub spai_mtime: Option<SystemTime>,
    /// Sliding window quota telemetry (5m/1h/5h session sliding tokens + upstream provider quota).
    pub quota: Option<crate::slices::telemetry::quota_live::QuotaTelemetry>,
}

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
                // Animation done: force a pane re-query so the status line returns
                // to the normal tab→pane binding description.
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

        // If switching between Status and Skills, notify Pi session via request file
        if let Some(path) = &self.snapshot_path {
            let tab_id = match tab {
                Tab::Status => "status",
                Tab::Skills => "skills",
                Tab::Zen | Tab::Mcp => return,
            };

            let col = if let Some(snap) = &self.snapshot {
                if let Some(hits) = &snap.tab_hits {
                    if let Some(hit) = hits.iter().find(|h| h.id == tab_id) {
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

        // 1. Refresh Herdr panes list
        self.panes = self.herdr_client.list_panes();

        // 2. Resolve target Pi pane if not explicit
        if !self.explicit_snapshot {
            let current_tab = self.target_tab_id.as_deref();
            let own_pane = self.own_pane_id.as_deref().unwrap_or("");

            // Find Pi agent in the SAME tab. Strict detection: the herdr
            // agent field or the distinctive 'π' glyph pi stamps in its
            // title. A plain "pi" substring also matched unrelated titles
            // ("...api...", shell panes whose title shows a ".../pi/..."
            // project path) and bound the sidebar to a foreign pane — with
            // multiple agents in a monorepo that displayed the wrong
            // session.
            let found_pane = self
                .panes
                .iter()
                .filter(|p| {
                    let same_tab = current_tab.is_none_or(|tid| p.tab_id.as_deref() == Some(tid));
                    let not_self = p.pane_id.as_str() != own_pane;
                    let is_pi = p.agent.as_deref() == Some("pi")
                        || p.terminal_title_stripped
                            .as_deref()
                            .is_some_and(|t| t.contains('π'))
                        || p.terminal_title.as_deref().is_some_and(|t| t.contains('π'));
                    same_tab && not_self && is_pi
                })
                // Multiple agents can share a tab: prefer the focused pane,
                // then agent-reported panes.
                .max_by_key(|p| (p.focused.unwrap_or(false), p.agent.as_deref() == Some("pi")));

            if let Some(pi_pane) = found_pane {
                self.target_pane_id = Some(pi_pane.pane_id.clone());
                let candidate_path = snapshot_path_for_pane(&pi_pane.pane_id);
                self.snapshot_path = Some(candidate_path);
                self.live_session_id = pi_pane.session_id();
                self.refresh_status = format!(
                    "Tab: {} → Pane: {}",
                    current_tab.unwrap_or("?"),
                    pi_pane.pane_id
                );
            } else {
                // No Pi agent in this tab. Never fall back to a global
                // snapshot: with multiple agents running in a monorepo the
                // newest snapshot across all tabs is another agent's
                // session. Show the waiting state until an agent registers
                // here.
                self.target_pane_id = None;
                self.snapshot_path = None;
                self.refresh_status =
                    format!("Tab: {} (bez pi relace)", current_tab.unwrap_or("?"));
            }
        }

        // 3. Check snapshot mtime and reload
        // 4. Refresh live telemetry from the session JSONL (independent of snapshot)
        self.refresh_live(force);
        self.refresh_skills(force);
        self.refresh_mcp(force);
        self.refresh_spai(force);
        self.refresh_quota(force);

        // Auto-switch to MCP tab when MCP server is actively used
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
                // Snapshot file vanished (e.g. /reload wiped the state dir entry):
                // count misses, then clear the stale frame so the UI shows the
                // waiting state and re-resolves the pane binding next tick.
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

    /// Skills face data: parsed directly from the Pi session JSONL (the same
    /// independent source as the Status face), falling back to the TS sidecar
    /// when no live session file is resolvable.
    fn refresh_skills(&mut self, force: bool) {
        // Primary: derive skill state from the session JSONL tool-call trail.
        if let Some(t) = &self.live {
            if let Some(f) = &t.session_file {
                if let Ok(meta) = std::fs::metadata(f) {
                    let mtime = meta.modified().ok();
                    if force || self.skills_mtime != mtime || self.skills.is_none() {
                        self.skills_mtime = mtime;
                        self.skills =
                            crate::slices::telemetry::skills_live::parse_session_skills(f);
                    }
                    return;
                }
            }
        }

        // Fallback: structured skills sidecar (`<snapshot>.skills.json`) written
        // by the TS pi-sidebar extension.
        let Some(path) = &self.snapshot_path else {
            self.skills = None;
            return;
        };
        let skills_path = {
            let mut p = path.clone();
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("sidebar")
                .to_string();
            let base = name.strip_suffix(".json").unwrap_or(&name).to_string();
            p.set_file_name(format!("{}.skills.json", base));
            p
        };

        let Ok(meta) = std::fs::metadata(&skills_path) else {
            self.skills = None;
            return;
        };
        let mtime = meta.modified().ok();
        if !force && self.skills.is_some() && mtime == self.skills_mtime {
            return; // unchanged
        }
        self.skills_mtime = mtime;
        self.skills =
            crate::slices::telemetry::skills::SkillSnapshotFile::read_from_file(&skills_path);
    }

    /// Refresh MCP server usage telemetry from Pi session JSONL
    fn refresh_mcp(&mut self, force: bool) {
        if let Some(t) = &self.live {
            if let Some(f) = &t.session_file {
                if let Ok(meta) = std::fs::metadata(f) {
                    let mtime = meta.modified().ok();
                    if force || self.mcp_mtime != mtime || self.mcp.is_none() {
                        self.mcp_mtime = mtime;
                        self.mcp = crate::slices::telemetry::mcp_live::parse_mcp_session(f);
                    }
                    return;
                }
            }
        }
        self.mcp = None;
    }

    /// Refresh SPAI task ledger directly from `docs/spai/.index.json`.
    fn refresh_spai(&mut self, force: bool) {
        let cwd = self.live.as_ref().map(|l| l.cwd.as_str()).or_else(|| {
            self.panes
                .iter()
                .find(|p| self.target_pane_id.as_deref() == Some(p.pane_id.as_str()))
                .and_then(|p| p.cwd.as_deref())
        });

        let index_path = crate::slices::telemetry::spai_live::find_spai_index_path(cwd);
        let Some(path) = index_path else {
            self.spai = None;
            return;
        };

        let Ok(meta) = std::fs::metadata(&path) else {
            self.spai = None;
            return;
        };
        let mtime = meta.modified().ok();
        if !force && self.spai.is_some() && mtime == self.spai_mtime {
            return; // unchanged
        }
        self.spai_mtime = mtime;
        self.spai = crate::slices::telemetry::spai_live::parse_spai_index(&path);
    }

    /// Refresh sliding window quota indicators.
    fn refresh_quota(&mut self, _force: bool) {
        let mut q = crate::slices::telemetry::quota_live::QuotaTelemetry::default();

        // 1. Fetch live Antigravity quota from upstream
        q.antigravity = crate::slices::telemetry::quota_live::fetch_antigravity_live_quota();

        // 2. Compute session sliding token consumption windows
        if let Some(t) = &self.live {
            if let Some(f) = &t.session_file {
                let (w5m, w1h, w5h) =
                    crate::slices::telemetry::quota_live::compute_session_sliding_windows(f);
                q.session_sliding_5m_tokens = w5m;
                q.session_sliding_1h_tokens = w1h;
                q.session_sliding_5h_tokens = w5h;
            }
        }

        self.quota = Some(q);
    }

    /// Apply a freshly-read snapshot and detect session lifecycle transitions
    /// (reload → live=false → new session with reset revision). Each transition
    /// drives the header refresh animation.
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
            // /reload or session end: keep last frame, animate the header
            self.refresh_timer = 24;
            self.refresh_progress = 0.0;
            self.refresh_status = "Pi relace se obnovuje…".to_string();
        } else if came_back || restarted {
            // New session instance took over: pulse the header and reset scroll
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

    /// Re-read live telemetry from the Pi session JSONL when it changed on disk.
    fn refresh_live(&mut self, force: bool) {
        // Try herdr-reported session id first; fall back to the newest session
        // JSONL on disk so telemetry works without agentSession reporting.
        let herdr_session = self.live_session_id.clone().filter(|s| s.len() >= 8);

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
            // No herdr-reported session id: scope strictly to the bound
            // pane's project folder. The old cross-project "newest anywhere"
            // scan displayed another agent's session when multiple agents
            // run in a monorepo.
            let file = crate::slices::telemetry::find_newest_session_scoped(pane_cwd.as_deref());
            self.load_live_from(file, "", force);
        }
    }

    /// Load telemetry from a located session file, honoring mtime-based caching.
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
            return; // unchanged
        }
        self.live_session_mtime = mtime;
        if let Some(t) = crate::slices::telemetry::parse_session(&file, session_id) {
            self.live = Some(t);
        }
    }
}
