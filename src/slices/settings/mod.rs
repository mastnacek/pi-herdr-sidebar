//! S.P.A.I. & OpenRouter Vectorization Settings slice.
pub mod actions;
pub mod cards_view;
pub mod models;
pub mod picker_view;
pub mod state;
pub mod storage;
pub mod view;

pub use actions::{classify_facets_all, vectorize_all_records};
pub use models::{ModelInfo, filter_models};
pub use state::{ModelTarget, SettingsField, SettingsState};
pub use view::render_settings_tab;
