//! S.P.A.I. Settings & Vectorization slice.
pub mod actions;
pub mod state;
pub mod view;

pub use actions::{classify_facets_all, vectorize_all_records};
pub use state::{SettingsField, SettingsState};
pub use view::render_settings_tab;
