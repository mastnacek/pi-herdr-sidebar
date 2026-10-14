//! Background semantic-search job state and polling.
//!
//! Split out of [`super::state`] to keep that file below the repository's
//! 400-line hard cap. The public types are re-exported from `state`, so
//! existing `super::state::{SemanticJob, …}` imports remain valid.
use super::state::ScratchState;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

/// One semantic-search job, run in a background thread (the embedding request
/// is network I/O). Progress reports land through the receiver and are shown
/// as the same spinner + `[█░] step/total (pct%)` bar the Settings tab uses
/// for vectorization.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SemanticProgress {
    #[default]
    Idle,
    /// Embedding request + cosine scan in progress: step/total/label.
    Running {
        step: usize,
        total: usize,
        label: String,
    },
}

/// A running semantic search: the receiver carries [`SemanticProgress`]
/// snapshots (step/total/label) and finally the allowed record paths.
#[derive(Debug)]
pub struct SemanticJob {
    pub receiver: Receiver<SemanticMessage>,
    /// Last reported progress (rendered as the settings-style bar).
    #[allow(dead_code)]
    pub progress: SemanticProgress,
    /// Spinner frame, advanced every tick while the job runs.
    pub spinner_tick: usize,
}

#[derive(Debug)]
pub enum SemanticMessage {
    Progress {
        step: usize,
        total: usize,
        label: String,
    },
    Done {
        allowed: std::collections::HashSet<PathBuf>,
        label: String,
    },
    Error(String),
}

impl ScratchState {
    /// Drains the background semantic-search job: progress snapshots update
    /// the bar, a finished result becomes the path-keyed allow-set (and the
    /// cursor jumps to the first match). Passive — called from the tick.
    pub fn poll_semantic(&mut self) {
        self.semantic_progress = SemanticProgress::Idle;
        let Some(job) = &mut self.semantic_job else {
            return;
        };
        job.spinner_tick = job.spinner_tick.wrapping_add(1);
        while let Ok(msg) = job.receiver.try_recv() {
            match msg {
                SemanticMessage::Progress { step, total, label } => {
                    self.semantic_progress = SemanticProgress::Running { step, total, label };
                }
                SemanticMessage::Done { allowed, label } => {
                    self.semantic_allowed = Some(allowed);
                    self.semantic_job = None;
                    self.semantic_progress = SemanticProgress::Idle;
                    self.jump_to_first_visible();
                    self.last_summary = Some(label);
                    return;
                }
                SemanticMessage::Error(e) => {
                    self.semantic_job = None;
                    self.semantic_progress = SemanticProgress::Idle;
                    self.last_summary = Some(format!("⚠ {}", e));
                    return;
                }
            }
        }
        // Still running without fresh progress? Keep the last label ticking.
        if self.semantic_progress == SemanticProgress::Idle {
            self.semantic_progress = SemanticProgress::Running {
                step: 0,
                total: 1,
                label: "Sémantické hledání…".to_string(),
            };
        }
    }

    /// Is a semantic search running? (Footer shows the progress bar.)
    pub fn semantic_running(&self) -> bool {
        self.semantic_job.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn poll_semantic_drains_progress_and_done() {
        let (tx, rx) = mpsc::channel::<SemanticMessage>();
        let mut s = ScratchState::default();
        s.semantic_job = Some(SemanticJob {
            receiver: rx,
            progress: SemanticProgress::Idle,
            spinner_tick: 0,
        });

        tx.send(SemanticMessage::Progress {
            step: 1,
            total: 4,
            label: " Embed…".into(),
        })
        .unwrap();
        s.poll_semantic();
        assert!(s.semantic_running(), "job continues");
        assert_eq!(
            s.semantic_progress,
            SemanticProgress::Running {
                step: 1,
                total: 4,
                label: " Embed…".into()
            }
        );

        // Done: the allow-set lands (path-keyed), cursor jumps to first visible.
        let mut allowed = std::collections::HashSet::new();
        allowed.insert(PathBuf::from("/tmp/x.md"));
        tx.send(SemanticMessage::Done {
            allowed,
            label: "2 shod".into(),
        })
        .unwrap();
        s.poll_semantic();
        assert!(!s.semantic_running());
        assert_eq!(s.semantic_allowed.as_ref().map(|a| a.len()), Some(1));
        assert_eq!(s.last_summary.as_deref(), Some("2 shod"));
    }

    #[test]
    fn poll_semantic_reports_errors_and_clears_the_job() {
        let (tx, rx) = mpsc::channel::<SemanticMessage>();
        let mut s = ScratchState::default();
        s.semantic_job = Some(SemanticJob {
            receiver: rx,
            progress: SemanticProgress::Idle,
            spinner_tick: 0,
        });
        tx.send(SemanticMessage::Error("síť neběží".into()))
            .unwrap();
        s.poll_semantic();
        assert!(!s.semantic_running());
        assert!(s.last_summary.unwrap_or_default().contains("síť neběží"));
    }
}
