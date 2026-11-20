//! Background write of new Scratchpad records (`spai add`).
use crate::shared::spai_client::AddOutcome;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

/// One new record to write: its first line, the project folder, the text.
#[derive(Debug, Clone)]
pub struct CreateItem {
    pub start: usize,
    pub project_path: PathBuf,
    pub text: String,
}

/// Per record: its first line and what `add` answered.
pub type SaveResults = Vec<(usize, Result<AddOutcome, String>)>;

/// A running write. The buffer is read-only until it ends, so the line
/// indices in the results stay valid.
#[derive(Debug)]
pub struct SaveJob {
    pub rx: Receiver<SaveResults>,
    /// Edited records that were already rewritten (shown in the summary).
    pub updated: usize,
    pub errors: Vec<String>,
    pub spinner_tick: usize,
}
