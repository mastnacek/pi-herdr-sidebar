//! Plugin & skill usage statistics derived from pi's own session logs.
//!
//! * [`model`] — what is counted and how a tool maps to a plugin.
//! * [`parse`] — per-record parsing of the JSONL logs.
//! * [`scan`] — file discovery, fingerprinting, caching and the scan pass.
//! * [`state`] — the scan-hosting state used by the standalone overview window.
//!
//! Scans are always background work: [`spawn_scan`] returns immediately and the
//! UI polls [`ScanHandle::progress`].
pub mod model;
pub mod parse;
pub mod scan;
pub mod state;

pub use model::UsageStats;
pub use scan::{load_cached, scan_blocking, session_files, sessions_dir};

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Live counters for a running scan (safe to read from the UI thread).
#[derive(Debug, Default)]
pub struct ScanProgress {
    done: AtomicUsize,
    total: AtomicUsize,
    finished: AtomicBool,
}

impl ScanProgress {
    pub fn done(&self) -> usize {
        self.done.load(Ordering::Relaxed)
    }

    pub fn total(&self) -> usize {
        self.total.load(Ordering::Relaxed)
    }

    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }

    /// Completion in `0.0..=1.0`.
    pub fn ratio(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            (self.done() as f64 / total as f64).clamp(0.0, 1.0)
        }
    }

    pub(crate) fn finish(&self) {
        self.finished.store(true, Ordering::Relaxed);
    }
}

/// Handle to a background scan. Dropping it detaches the worker; the UI keeps
/// polling until [`ScanHandle::take_result`] yields the aggregate.
pub struct ScanHandle {
    progress: Arc<ScanProgress>,
    result: Arc<Mutex<Option<UsageStats>>>,
    error: Arc<Mutex<Option<String>>>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for ScanHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScanHandle")
            .field("finished", &self.progress.is_finished())
            .field("done", &self.progress.done())
            .field("total", &self.progress.total())
            .finish()
    }
}

impl ScanHandle {
    pub fn progress(&self) -> &ScanProgress {
        &self.progress
    }

    /// Harvests the aggregate once the worker has finished, exactly once.
    pub fn take_result(&mut self) -> Option<Result<UsageStats, String>> {
        if !self.progress.is_finished() {
            return None;
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        if let Some(err) = self.error.lock().ok().and_then(|mut e| e.take()) {
            return Some(Err(err));
        }
        self.result.lock().ok().and_then(|mut r| r.take()).map(Ok)
    }
}

/// Starts a background scan.
///
/// Without `force` a cached aggregate is reused when the log fingerprint is
/// unchanged, which makes the common case nearly free.
pub fn spawn_scan(force: bool) -> ScanHandle {
    let progress = Arc::new(ScanProgress::default());
    let result = Arc::new(Mutex::new(None));
    let error = Arc::new(Mutex::new(None));

    let worker = {
        let progress = Arc::clone(&progress);
        let result = Arc::clone(&result);
        let error = Arc::clone(&error);
        std::thread::spawn(move || {
            let files = session_files();
            let fingerprint = scan::fingerprint_of(&files);
            progress.total.store(files.len(), Ordering::Relaxed);

            if !force {
                if let Some(cached) = load_cached(&fingerprint) {
                    progress.done.store(files.len(), Ordering::Relaxed);
                    progress.finish();
                    if let Ok(mut slot) = result.lock() {
                        *slot = Some(cached);
                    }
                    return;
                }
            }

            match scan_blocking(&progress) {
                Ok(stats) => {
                    scan::store_cached(&fingerprint, &stats);
                    if let Ok(mut slot) = result.lock() {
                        *slot = Some(stats);
                    }
                }
                Err(err) => {
                    progress.finish();
                    if let Ok(mut slot) = error.lock() {
                        *slot = Some(err);
                    }
                }
            }
        })
    };

    ScanHandle {
        progress,
        result,
        error,
        join: Some(worker),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_ratio_handles_empty_work() {
        let p = ScanProgress::default();
        assert_eq!(p.ratio(), 0.0);
        assert!(!p.is_finished());
        p.total.store(4, Ordering::Relaxed);
        p.done.store(1, Ordering::Relaxed);
        assert!((p.ratio() - 0.25).abs() < f64::EPSILON);
        p.finish();
        assert!(p.is_finished());
    }
}
