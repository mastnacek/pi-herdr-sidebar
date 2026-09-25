//! State for the standalone plugin-usage overview window.
//!
//! Kept separate from the Shortcuts tab: the tab only documents keybindings and
//! never scans anything. The overview runs in its own process/pane and owns the
//! background scan handle, plus the scroll position of each panel.
use super::{ScanHandle, ScanProgress, UsageStats};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    #[default]
    Plugins = 0,
    Skills = 1,
    Commands = 2,
    Tools = 3,
}

impl Panel {
    pub const ALL: [Panel; 4] = [Panel::Plugins, Panel::Skills, Panel::Commands, Panel::Tools];
    pub const COUNT: usize = 4;

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn title(self) -> &'static str {
        match self {
            Panel::Plugins => "Pluginy",
            Panel::Skills => "Skilly",
            Panel::Commands => "Příkazy",
            Panel::Tools => "Nástroje",
        }
    }
}

#[derive(Debug, Default)]
pub struct PanelState {
    pub scroll: u16,
    pub view_h: u16,
    pub rows: u16,
}

#[derive(Debug, Default)]
pub struct UsageOverview {
    pub stats: Option<UsageStats>,
    pub status: Option<String>,
    panels: [PanelState; Panel::COUNT],
    scan: Option<ScanHandle>,
    /// Currently focused panel for keyboard navigation.
    focus: Panel,
}

impl UsageOverview {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a scan unless one is already running.
    ///
    /// `force` ignores the on-disk cache and re-reads every session file.
    pub fn ensure_scan(&mut self, force: bool) {
        if self.scan.is_some() {
            return;
        }
        if force {
            self.stats = None;
            self.status = Some("Přeskenuji session logy…".to_string());
        }
        self.scan = Some(super::spawn_scan(force));
    }

    /// Harvests a finished scan; call once per UI tick.
    pub fn poll_scan(&mut self) {
        let Some(handle) = self.scan.as_mut() else {
            return;
        };
        let Some(result) = handle.take_result() else {
            return;
        };
        self.scan = None;
        match result {
            Ok(stats) => {
                self.status = Some(format!(
                    "Proskenováno {} souborů za {} ms",
                    stats.files, stats.elapsed_ms
                ));
                self.stats = Some(stats);
            }
            Err(err) => self.status = Some(format!("Skenování selhalo: {err}")),
        }
    }

    /// Progress of a running scan, if any.
    pub fn scan_progress(&self) -> Option<&ScanProgress> {
        self.scan.as_ref().map(|h| h.progress())
    }

    pub fn is_scanning(&self) -> bool {
        self.scan.is_some()
    }

    /// Returns the state for the currently focused panel.
    fn panel_mut(&mut self) -> &mut PanelState {
        &mut self.panels[self.focus.index()]
    }

    fn panel(&self) -> &PanelState {
        &self.panels[self.focus.index()]
    }

    /// Returns the state for a specific panel.
    pub fn panel_state(&self, panel: Panel) -> &PanelState {
        &self.panels[panel.index()]
    }

    /// Returns mutable state for a specific panel.
    pub fn panel_state_mut(&mut self, panel: Panel) -> &mut PanelState {
        &mut self.panels[panel.index()]
    }

    /// Currently focused panel.
    pub fn focus(&self) -> Panel {
        self.focus
    }

    /// Sets the focused panel.
    pub fn set_focus(&mut self, panel: Panel) {
        self.focus = panel;
    }

    /// Cycles focus to the next panel.
    pub fn focus_next(&mut self) {
        self.focus = match self.focus {
            Panel::Plugins => Panel::Skills,
            Panel::Skills => Panel::Commands,
            Panel::Commands => Panel::Tools,
            Panel::Tools => Panel::Plugins,
        };
    }

    /// Cycles focus to the previous panel.
    pub fn focus_prev(&mut self) {
        self.focus = match self.focus {
            Panel::Plugins => Panel::Tools,
            Panel::Skills => Panel::Plugins,
            Panel::Commands => Panel::Skills,
            Panel::Tools => Panel::Commands,
        };
    }

    /// Last scrollable position for the focused panel, given the geometry cached by the renderer.
    pub fn max_scroll(&self) -> u16 {
        self.panel().rows.saturating_sub(self.panel().view_h)
    }

    /// Moves the viewport by `delta` rows, clamped to the document.
    pub fn scroll_by(&mut self, delta: i32) {
        let panel = self.panel_mut();
        let next = panel.scroll as i32 + delta;
        panel.scroll = next.clamp(0, panel.rows.saturating_sub(panel.view_h) as i32) as u16;
    }

    /// Pages by one screen (one row of overlap so nothing is skipped visually).
    pub fn page(&mut self, down: bool) {
        let panel = self.panel_mut();
        let page = panel.view_h.saturating_sub(1).max(1) as i32;
        let next = panel.scroll as i32 + if down { page } else { -page };
        panel.scroll = next.clamp(0, panel.rows.saturating_sub(panel.view_h) as i32) as u16;
    }

    pub fn scroll_home(&mut self) {
        self.panel_mut().scroll = 0;
    }

    pub fn scroll_end(&mut self) {
        let panel = self.panel_mut();
        panel.scroll = panel.rows.saturating_sub(panel.view_h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Geometry as the renderer would have left it: 100 rows in a 20-row view.
    fn scrolled() -> UsageOverview {
        let mut state = UsageOverview::new();
        state.panel_state_mut(Panel::Plugins).rows = 100;
        state.panel_state_mut(Panel::Plugins).view_h = 20;
        state
    }

    #[test]
    fn starts_empty_and_polling_without_a_scan_is_a_noop() {
        let mut state = UsageOverview::new();
        assert!(state.stats.is_none());
        assert!(state.scan_progress().is_none());
        assert!(!state.is_scanning());
        state.poll_scan();
        assert!(state.stats.is_none());
    }

    #[test]
    fn scroll_is_clamped_to_the_document() {
        let mut state = scrolled();
        state.scroll_by(-5);
        assert_eq!(
            state.panel_state(Panel::Plugins).scroll,
            0,
            "cannot scroll above the first row"
        );
        state.scroll_by(10_000);
        assert_eq!(
            state.panel_state(Panel::Plugins).scroll,
            80,
            "cannot scroll past rows - view height"
        );
    }

    #[test]
    fn page_moves_by_one_screen_and_clamps() {
        let mut state = scrolled();
        state.page(true);
        assert_eq!(
            state.panel_state(Panel::Plugins).scroll,
            19,
            "one screen minus the overlap row"
        );
        state.page(false);
        assert_eq!(state.panel_state(Panel::Plugins).scroll, 0);
        state.page(false);
        assert_eq!(
            state.panel_state(Panel::Plugins).scroll,
            0,
            "clamped at the top"
        );
    }

    #[test]
    fn home_and_end_jump_to_the_bounds() {
        let mut state = scrolled();
        state.scroll_end();
        assert_eq!(state.panel_state(Panel::Plugins).scroll, 80);
        state.scroll_home();
        assert_eq!(state.panel_state(Panel::Plugins).scroll, 0);
    }

    #[test]
    fn a_document_that_fits_does_not_scroll() {
        let mut state = UsageOverview::new();
        state.panel_state_mut(Panel::Plugins).rows = 10;
        state.panel_state_mut(Panel::Plugins).view_h = 20;
        assert_eq!(state.max_scroll(), 0);
        state.scroll_by(5);
        assert_eq!(state.panel_state(Panel::Plugins).scroll, 0);
        state.scroll_end();
        assert_eq!(state.panel_state(Panel::Plugins).scroll, 0);
    }

    #[test]
    fn focus_cycles_through_panels() {
        let mut state = UsageOverview::new();
        assert_eq!(state.focus(), Panel::Plugins);
        state.focus_next();
        assert_eq!(state.focus(), Panel::Skills);
        state.focus_next();
        assert_eq!(state.focus(), Panel::Commands);
        state.focus_next();
        assert_eq!(state.focus(), Panel::Tools);
        state.focus_next();
        assert_eq!(state.focus(), Panel::Plugins);
        state.focus_prev();
        assert_eq!(state.focus(), Panel::Tools);
    }

    #[test]
    fn each_panel_has_independent_scroll() {
        let mut state = UsageOverview::new();
        state.panel_state_mut(Panel::Plugins).rows = 100;
        state.panel_state_mut(Panel::Plugins).view_h = 20;
        state.panel_state_mut(Panel::Skills).rows = 50;
        state.panel_state_mut(Panel::Skills).view_h = 20;

        state.scroll_end(); // plugins at 80
        state.focus_next(); // skills
        state.scroll_end(); // skills at 30

        assert_eq!(state.panel_state(Panel::Plugins).scroll, 80);
        assert_eq!(state.panel_state(Panel::Skills).scroll, 30);
    }
}
