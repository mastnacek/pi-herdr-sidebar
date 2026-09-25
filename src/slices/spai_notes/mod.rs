//! Spai Notes Slice
pub mod autocomplete;
pub mod dialog_state;
pub mod dialog_views;
pub mod discovery;
pub mod external_editor;
pub mod input_highlighter;
pub mod note;
pub mod state;
pub mod state_creation;
pub mod state_edit;
pub mod storage_format;
pub mod text_cursor;
pub mod text_layout;
pub mod time_utils;
pub mod view;
pub mod viewer_content;

pub use state::SpaiNotesState;
pub use view::render_spai_notes_tab;
