//! Dialog models for SPAI notes (creation with autocomplete and inline editing).
use super::autocomplete::ProjectSuggestion;
use super::note::SpaiType;
use super::similarity::SimilarNoteMatch;
use std::sync::mpsc::Receiver;
use std::time::Instant;

pub struct NoteCreationDialog {
    pub active: bool,
    pub title_input: String,
    pub cursor: usize,
    pub selected_kind: SpaiType,
    pub type_selection: usize,
    pub autocomplete_active: bool,
    pub autocomplete_selected: usize,
    pub suggestions: Vec<ProjectSuggestion>,
    pub last_keystroke: Option<Instant>,
    pub debounced_query: String,
    pub debounced_matches: Vec<SimilarNoteMatch>,
    pub candidate_vector: Option<Vec<f64>>,
    pub is_evaluating_vector: bool,
    pub vector_evaluated: bool,
    pub dedup_receiver: Option<Receiver<Vec<SimilarNoteMatch>>>,
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
            last_keystroke: None,
            debounced_query: String::new(),
            debounced_matches: Vec::new(),
            candidate_vector: None,
            is_evaluating_vector: false,
            vector_evaluated: false,
            dedup_receiver: None,
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
            .field("debounced_matches", &self.debounced_matches)
            .field("is_evaluating_vector", &self.is_evaluating_vector)
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
            last_keystroke: self.last_keystroke,
            debounced_query: self.debounced_query.clone(),
            debounced_matches: self.debounced_matches.clone(),
            candidate_vector: self.candidate_vector.clone(),
            is_evaluating_vector: self.is_evaluating_vector,
            vector_evaluated: self.vector_evaluated,
            dedup_receiver: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditField {
    #[default]
    Title,
    Body,
}

#[derive(Debug, Clone, Default)]
pub struct NoteEditDialog {
    pub active: bool,
    pub title_input: String,
    pub body_input: String,
    pub field: EditField,
    /// Cursor position as a char index inside the focused field.
    pub cursor: usize,
    /// Manual body viewport offset, used while `body_follow` is `false`.
    pub body_scroll: u16,
    /// When `true` the body viewport auto-scrolls so the cursor stays visible.
    /// Flipped off by manual scrolling (PgUp/PgDn) and back on by any edit or
    /// cursor movement.
    pub body_follow: bool,
}
