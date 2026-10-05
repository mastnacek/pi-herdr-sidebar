//! S.P.A.I. & OpenRouter Vectorization Settings slice.
pub mod actions;
pub mod cards_view;
pub mod classify_prompt;
pub mod classify_service;
pub mod models;
pub mod models_cache;
pub mod picker_view;
pub mod state;
pub mod state_cycling;
pub mod storage;
pub mod vector_service;
pub mod view;

pub use actions::{
    classify_all_facets, classify_missing_facets, vectorize_all_records, vectorize_missing_records,
};
pub use classify_service::collect_taxonomy;
pub use models::{filter_models, ModelInfo};
pub use state::{AsyncProgress, ModelTarget, SettingsField, SettingsState};
pub use vector_service::{load_project_vectors, ProjectVectorStore};
pub use view::render_settings_tab;
