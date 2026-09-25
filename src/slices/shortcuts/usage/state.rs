//! State for the standalone plugin-usage overview window.
//!
//! Kept separate from the Shortcuts tab: the tab only documents keybindings and
//! never scans anything. The overview runs in its own process/pane and owns the
//! background scan handle, plus the scroll position of its document.
use super::{ScanHandle, ScanProgress, UsageStats};

#[derive(Debug, Default)]
pub struct UsageOverview {
    pub stats: Option<UsageStats>,
    pub status: Option<String>,
    /// First visible document row.
    pub scroll: u16,
    /// Height of the scrolling area — cached by the renderer so paging is exact.
    pub view_h: u16,
    /// Total document rows — cached by the renderer so `End` is exact.
    pub rows: u16,
    scan: Option<ScanHandle>,
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

    /// Last scrollable position, given the geometry cached by the renderer.
    pub fn max_scroll(&self) -> u16 {
        self.rows.saturating_sub(self.view_h)
    }

    /// Moves the viewport by `delta` rows, clamped to the document.
    pub fn scroll_by(&mut self, delta: i32) {
        let next = self.scroll as i32 + delta;
        self.scroll = next.clamp(0, self.max_scroll() as i32) as u16;
    }

    /// Pages by one screen (one row of overlap so nothing is skipped visually).
    pub fn page(&mut self, down: bool) {
        let page = self.view_h.saturating_sub(1).max(1) as i32;
        self.scroll_by(if down { page } else { -page });
    }

    pub fn scroll_home(&mut self) {
        self.scroll = 0;
    }

    pub fn scroll_end(&mut self) {
        self.scroll = self.max_scroll();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Geometry as the renderer would have left it: 100 rows in a 20-row view.
    fn scrolled() -> UsageOverview {
        let mut state = UsageOverview::new();
        state.rows = 100;
        state.view_h = 20;
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
        assert_eq!(state.scroll, 0, "cannot scroll above the first row");
        state.scroll_by(10_000);
        assert_eq!(state.scroll, 80, "cannot scroll past rows - view height");
    }

    #[test]
    fn page_moves_by_one_screen_and_clamps() {
        let mut state = scrolled();
        state.page(true);
        assert_eq!(state.scroll, 19, "one screen minus the overlap row");
        state.page(false);
        assert_eq!(state.scroll, 0);
        state.page(false);
        assert_eq!(state.scroll, 0, "clamped at the top");
    }

    #[test]
    fn home_and_end_jump_to_the_bounds() {
        let mut state = scrolled();
        state.scroll_end();
        assert_eq!(state.scroll, 80);
        state.scroll_home();
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn a_document_that_fits_does_not_scroll() {
        let mut state = UsageOverview::new();
        state.rows = 10;
        state.view_h = 20;
        assert_eq!(state.max_scroll(), 0);
        state.scroll_by(5);
        assert_eq!(state.scroll, 0);
        state.scroll_end();
        assert_eq!(state.scroll, 0);
    }
}
