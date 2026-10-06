//! Scratchpad (docs/scratchpad_mode_plan.md): full-area writing mode where
//! every SPAI-marked line becomes a physical file on Ctrl+S.
//!
//! Modules:
//! - [`line_model`] — `ScratchLine`/`LineOrigin`, mark detection, record spans
//! - [`state`] — mode, scope, buffer, cursor, filters, dedup popup state
//! - [`buffer`] — Edit-mode text operations on the buffer
//! - [`visibility`] — filter helpers, record navigation
//! - [`draft`] — unsaved-lines autosave in `HERDR_PLUGIN_STATE_DIR`
//! - [`save`] — Ctrl+S pipeline + `u` (undo batch)
//! - [`dedup`] / [`dedup_popup`] — Ctrl+D engine glue and popup rendering
//! - [`mention`] / [`mention_popup`] — `@` project autocomplete + rendering
//! - [`filter`] — token parser, fuzzy scoring, AND-composition
//! - [`semantic`] — `~` filter: one embedding + cosine over stored vectors
//! - [`scope`] — New / Project / All loading, record opening, status cycling
//! - [`view`] / [`footer`] / [`help`] — rendering (buffer, footer, `?` overlay)
//!
//! Keys are wired in the view slice (`view/keys/scratch.rs`), the same way
//! the other dialogs are; the slice owns the logic and stays import-free
//! from sibling slices (VSA rule).
pub mod buffer;
pub mod dedup;
pub mod dedup_popup;
pub mod draft;
pub mod filter;
pub mod footer;
pub mod help;
pub mod line_model;
pub mod mention;
pub mod mention_popup;
pub mod save;
pub mod scope;
pub mod semantic;
pub mod state;
pub mod view;
pub mod visibility;

/// Hook called by the event loop's tick so background dedup results land
/// without any timer-driven start (plan §3: "no automatic request").
pub fn poll_background(state: &mut super::state::SpaiNotesState) {
    state.scratch.poll_dedup();
}
