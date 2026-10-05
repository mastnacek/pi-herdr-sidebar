//! Dialog rendering for SPAI notes (creation and inline editing).

mod creation;
pub mod creation_parts;
mod edit;

#[cfg(test)]
mod tests;

pub use creation::render_creation_dialog;
pub use edit::render_edit_dialog;
