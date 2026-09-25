//! State for the standalone plugin-usage overview window.
//!
//! Kept separate from the Shortcuts tab: the tab only documents keybindings and
//! never scans anything. The overview runs in its own process/pane and owns the
//! background scan handle.
use super::{ScanHandle, ScanProgress, UsageStats};

#[derive(Debug, Default)]
pub struct UsageOverview {
    pub stats: Option<UsageStats>,
    pub status: Option<String>,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty_and_polling_without_a_scan_is_a_noop() {
        let mut state = UsageOverview::new();
        assert!(state.stats.is_none());
        assert!(state.scan_progress().is_none());
        assert!(!state.is_scanning());
        state.poll_scan();
        assert!(state.stats.is_none());
    }
}
