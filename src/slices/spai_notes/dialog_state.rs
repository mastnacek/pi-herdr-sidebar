//! Dialog models for SPAI notes (creation with autocomplete, inline editor
//! with Normal/Insert modes, and the on-demand dedup panel).
use super::autocomplete::ProjectSuggestion;
use super::note::SpaiType;
use super::similarity::SimilarNoteMatch;
use std::sync::mpsc::Receiver;

pub struct NoteCreationDialog {
    pub active: bool,
    pub title_input: String,
    pub cursor: usize,
    pub selected_kind: SpaiType,
    pub type_selection: usize,
    pub autocomplete_active: bool,
    pub autocomplete_selected: usize,
    pub suggestions: Vec<ProjectSuggestion>,
}

impl Default for NoteCreationDialog {
    fn default() -> Self {
        Self {
            active: false,
            title_input: String::new(),
            cursor: 0,
            selected_kind: SpaiType::Todo,
            type_selection: 0,
            autocomplete_active: false,
            autocomplete_selected: 0,
            suggestions: Vec::new(),
        }
    }
}

impl std::fmt::Debug for NoteCreationDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoteCreationDialog")
            .field("active", &self.active)
            .field("title_input", &self.title_input)
            .field("cursor", &self.cursor)
            .field("selected_kind", &self.selected_kind)
            .finish()
    }
}

impl Clone for NoteCreationDialog {
    fn clone(&self) -> Self {
        Self {
            active: self.active,
            title_input: self.title_input.clone(),
            cursor: self.cursor,
            selected_kind: self.selected_kind,
            type_selection: self.type_selection,
            autocomplete_active: self.autocomplete_active,
            autocomplete_selected: self.autocomplete_selected,
            suggestions: self.suggestions.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditField {
    #[default]
    Title,
    Body,
}

/// Vim-like editor modes. The integrated editor opens in **Normal** (read),
/// `i`/`a`/`A` switch to Insert, `Esc` returns to Normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditMode {
    #[default]
    Normal,
    Insert,
}

#[derive(Debug, Clone, Default)]
pub struct NoteEditDialog {
    pub active: bool,
    pub title_input: String,
    pub body_input: String,
    pub field: EditField,
    pub mode: EditMode,
    /// `true` once any text was changed since the dialog opened (or last save).
    pub dirty: bool,
    /// Discard confirmation for closing with unsaved changes: first Esc arms,
    /// second Esc discards, any other key disarms.
    pub confirm_discard: bool,
    /// Cursor position as a char index inside the focused field.
    pub cursor: usize,
    /// Manual body viewport offset, used while `body_follow` is `false`.
    pub body_scroll: u16,
    /// When `true` the body viewport auto-scrolls so the cursor stays visible.
    /// Flipped off by manual scrolling (PgUp/PgDn) and back on by any edit or
    /// cursor movement.
    pub body_follow: bool,
}

/// On-demand duplicate check (`Ctrl+D`), shared by the creation dialog and the
/// integrated editor. Nothing runs by itself: the panel only polls a finished
/// background computation; no timer, no network without an explicit shortcut.
#[derive(Default)]
pub struct DedupPanel {
    pub visible: bool,
    pub is_evaluating: bool,
    pub matches: Vec<SimilarNoteMatch>,
    pub receiver: Option<Receiver<Vec<SimilarNoteMatch>>>,
}

impl Clone for DedupPanel {
    fn clone(&self) -> Self {
        Self {
            visible: self.visible,
            is_evaluating: self.is_evaluating,
            matches: self.matches.clone(),
            receiver: None,
        }
    }
}

impl std::fmt::Debug for DedupPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DedupPanel")
            .field("visible", &self.visible)
            .field("is_evaluating", &self.is_evaluating)
            .field("matches", &self.matches.len())
            .finish()
    }
}
