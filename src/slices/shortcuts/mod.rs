//! Shortcuts slice — the "which key runs which plugin" face of the sidebar.
//!
//! * [`keys`] — reads the user's Herdr `config.toml` bindings and merges them
//!   with the documented defaults (Herdr has no CLI that dumps keybindings).
//! * [`model`] — state of the Shortcuts tab.
//! * [`view`] — the Shortcuts tab itself, which only documents chords.
//! * [`usage`] — scans pi's session logs for plugin/skill usage (background,
//!   cached, with its own state).
//! * [`modal`] — renders the usage panel.
//! * [`overview`] — the standalone overview window entrypoint
//!   (`pi_sidebar usage`), bound to its own Herdr chord. It deliberately renders
//!   *only* the overview, with no sidebar chrome.
pub mod keys;
pub mod modal;
pub mod model;
pub mod overview;
pub mod usage;
pub mod view;

pub use model::ShortcutsState;

/// The tab's renderer, exported as the slice's public face so the view layer
/// does not have to know this slice's internal module layout.
pub use view::render_shortcuts_tab;
