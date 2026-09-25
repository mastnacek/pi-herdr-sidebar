//! Dialog models for SPAI notes (creation with autocomplete and inline editing).
use super::autocomplete::ProjectSuggestion;
use super::note::SpaiType;

#[derive(Debug, Clone)]
pub struct NoteCreationDialog {
    pub active: bool,
    pub title_input: String,
    pub selected_kind: SpaiType,
    pub autocomplete_active: bool,
    pub autocomplete_selected: usize,
    pub suggestions: Vec<ProjectSuggestion>,
}

impl Default for NoteCreationDialog {
    fn default() -> Self {
        Self {
            active: false,
            title_input: String::new(),
            selected_kind: SpaiType::Todo,
            autocomplete_active: false,
            autocomplete_selected: 0,
            suggestions: Vec::new(),
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
