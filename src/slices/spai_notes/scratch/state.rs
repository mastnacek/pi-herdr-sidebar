//! Scratchpad state (plan §1, §4): mode, scope, buffer, cursor, dirty flag.
//!
//! The buffer is a flat list of [`ScratchLine`]s; a record is a marked line
//! plus its unmarked continuation lines (see [`super::line_model`]). Saved
//! lines are never editable in the buffer — changes go through Read actions
//! or the Notes editor, so text and file cannot diverge.
//!
//! Split for the line cap:
//! - [`super::buffer`] — Edit-mode text operations on the buffer
//! - [`super::visibility`] — filter helpers and record navigation
use super::filter::{FilterQuery, StatusFilter};
use super::line_model::{is_marked, ScratchLine};
use super::super::similarity::SimilarNoteMatch;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScratchMode {
    #[default]
    Edit,
    Read,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScratchScope {
    /// Only new lines (plus a restored draft).
    #[default]
    New,
    /// Records of the current project loaded as lines.
    Project,
    /// Records of all projects, loaded lazily.
    All,
}

impl ScratchScope {
    pub fn next(self) -> Self {
        match self {
            ScratchScope::New => ScratchScope::Project,
            ScratchScope::Project => ScratchScope::All,
            ScratchScope::All => ScratchScope::New,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ScratchScope::New => "Nové",
            ScratchScope::Project => "Projekt",
            ScratchScope::All => "Vše",
        }
    }
}

/// What the footer input line is currently collecting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScratchInput {
    FuzzyFilter,
    SemanticFilter,
}

/// `@` project autocomplete: anchored under the cursor line while a mention
/// token is being typed (the matches come from `SpaiNotesState.projects` at
/// render time, so this stays settings-free and tiny).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MentionPopup {
    pub anchor_line: usize,
    pub selected: usize,
}

/// Ctrl+D popup state for the line under the cursor (plan §3).
#[derive(Default)]
pub struct ScratchDedup {
    pub visible: bool,
    pub anchor_line: usize,
    pub is_evaluating: bool,
    pub matches: Vec<SimilarNoteMatch>,
    pub selected: usize,
    pub receiver: Option<Receiver<Vec<SimilarNoteMatch>>>,
}

impl Clone for ScratchDedup {
    fn clone(&self) -> Self {
        Self {
            visible: self.visible,
            anchor_line: self.anchor_line,
            is_evaluating: self.is_evaluating,
            matches: self.matches.clone(),
            selected: self.selected,
            receiver: None,
        }
    }
}

impl std::fmt::Debug for ScratchDedup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScratchDedup")
            .field("visible", &self.visible)
            .field("anchor_line", &self.anchor_line)
            .field("is_evaluating", &self.is_evaluating)
            .field("matches", &self.matches.len())
            .finish()
    }
}

#[derive(Debug, Default, Clone)]
pub struct ScratchState {
    pub open: bool,
    pub mode: ScratchMode,
    pub scope: ScratchScope,
    pub lines: Vec<ScratchLine>,
    /// Cursor as (line index, char index within that line).
    pub cursor_line: usize,
    pub cursor_char: usize,
    /// First visible line (virtualised rendering for the All scope).
    pub scroll: usize,
    pub dirty: bool,
    /// Unsaved New lines exist → closing asks first (Esc twice).
    pub confirm_close: bool,
    pub filter: Option<FilterQuery>,
    pub status_filter: StatusFilter,
    pub semantic_allowed: Option<Vec<usize>>,
    /// Footer input line (filter / semantic query) being typed.
    pub input_mode: Option<ScratchInput>,
    pub input_buffer: String,
    /// Files created by the last Ctrl+S batch — `u` in Read deletes them.
    pub last_batch: Vec<PathBuf>,
    /// Footer summary of the last save: `Uloženo 3 · herdr SPAI-014…`.
    pub last_summary: Option<String>,
    /// Ctrl+S warning when a fresh dedup result is similar: `⚠ similar: SPAI-009`.
    pub warn_similar: Option<String>,
    pub dedup: ScratchDedup,
    /// `@` autocomplete popup while a mention token is open, or `None`.
    pub mention: Option<MentionPopup>,
    /// Plugin state dir (draft storage); filled on open from the context.
    pub state_dir: Option<PathBuf>,
}

impl ScratchState {
    pub fn open(&mut self, state_dir: Option<PathBuf>) {
        self.open = true;
        self.mode = ScratchMode::Edit;
        self.state_dir = state_dir;
        self.dirty = false;
        self.confirm_close = false;
        self.last_batch.clear();
        self.last_summary = None;
        self.warn_similar = None;
        self.dedup = ScratchDedup::default();
        self.input_mode = None;
        self.input_buffer.clear();
        self.filter = None;
        self.semantic_allowed = None;
        self.status_filter = StatusFilter::All;
        self.mention = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.dedup = ScratchDedup::default();
        self.input_mode = None;
        self.input_buffer.clear();
        self.confirm_close = false;
        self.mention = None;
    }

    pub fn reset_for_scope(&mut self) {
        self.lines.clear();
        self.cursor_line = 0;
        self.cursor_char = 0;
        self.scroll = 0;
        self.dirty = false;
        self.confirm_close = false;
        self.filter = None;
        self.semantic_allowed = None;
        self.status_filter = StatusFilter::All;
        self.last_batch.clear();
        self.last_summary = None;
        self.warn_similar = None;
        self.dedup = ScratchDedup::default();
        self.mention = None;
    }

    // ── Cursor helpers ────────────────────────────────────────────

    pub fn current_line(&self) -> Option<&ScratchLine> {
        self.lines.get(self.cursor_line)
    }

    pub fn current_line_len(&self) -> usize {
        self.current_line()
            .map(|l| l.text.chars().count())
            .unwrap_or(0)
    }

    pub fn clamp_cursor(&mut self) {
        if self.lines.is_empty() {
            self.cursor_line = 0;
            self.cursor_char = 0;
            return;
        }
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }
        let len = self.current_line_len();
        self.cursor_char = self.cursor_char.min(len);
    }

    /// Is the line under the cursor editable? Empty buffer counts as editable.
    pub fn editable_at_cursor(&self) -> bool {
        match self.lines.get(self.cursor_line) {
            None => true,
            Some(l) => l.origin.is_editable(),
        }
    }

    /// Is the current line a marked record candidate (savable)?
    pub fn current_line_is_marked(&self) -> bool {
        self.current_line().map(|l| is_marked(&l.text)).unwrap_or(false)
    }

    // ── Dedup popup plumbing ──────────────────────────────────────

    /// Does a fresh dedup result exist for this session? (Ctrl+S shows only
    /// a footer warning — no prompt, plan §3.)
    pub fn fresh_similar_id(&self) -> Option<&str> {
        if !self.dedup.visible || self.dedup.matches.is_empty() {
            return None;
        }
        self.dedup.matches.first().map(|m| m.id.as_str())
    }

    /// Drains a finished background dedup computation. Purely passive.
    pub fn poll_dedup(&mut self) {
        if let Some(rx) = &self.dedup.receiver {
            if let Ok(matches) = rx.try_recv() {
                self.dedup.matches = matches;
                self.dedup.is_evaluating = false;
                self.dedup.receiver = None;
            }
        }
    }

    pub fn close_dedup(&mut self) {
        self.dedup = ScratchDedup::default();
    }

    // `@` autocomplete plumbing lives in `super::mention` (file-size cap).
}

/// Byte offset of a char index (UTF-8 safe).
pub(crate) fn byte_of_char(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}