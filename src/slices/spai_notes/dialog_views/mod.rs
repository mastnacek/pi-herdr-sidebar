//! Dialog rendering for SPAI notes (creation, editor, dedup overlay).
mod creation;
pub mod creation_parts;
mod dedup_overlay;
mod edit;

#[cfg(test)]
mod tests;

pub use creation::render_creation_dialog;
pub use edit::render_edit_dialog;
