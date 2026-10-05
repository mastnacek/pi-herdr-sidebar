//! State and models for S.P.A.I. & OpenRouter Vectorization settings.
use super::models::{
    fetch_openrouter_models, filter_models, load_cached_models, save_cached_models, ModelInfo,
};
use super::storage::{load_config_file, save_config_file};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsField {
    #[default]
    ApiKey,
    ChatModel,
    EmbeddingModel,
    SimilarityThreshold,
    VectorizeAction,
    ClassifyAction,
}

impl SettingsField {
    pub fn next(self) -> Self {
        match self {
            Self::ApiKey => Self::ChatModel,
            Self::ChatModel => Self::EmbeddingModel,
            Self::EmbeddingModel => Self::SimilarityThreshold,
            Self::SimilarityThreshold => Self::VectorizeAction,
            Self::VectorizeAction => Self::ClassifyAction,
            Self::ClassifyAction => Self::ApiKey,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::ApiKey => Self::ClassifyAction,
            Self::ChatModel => Self::ApiKey,
            Self::EmbeddingModel => Self::ChatModel,
            Self::SimilarityThreshold => Self::EmbeddingModel,
            Self::VectorizeAction => Self::SimilarityThreshold,
            Self::ClassifyAction => Self::VectorizeAction,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTarget {
    Chat,
    Embedding,
}

#[derive(Debug, Clone)]
pub struct SettingsState {
    pub api_key: String,
    pub chat_model: String,
    pub embedding_model: String,
    pub similarity_threshold: u8,
    pub selected_field: SettingsField,
    pub editing_api_key: bool,
    pub api_key_input: String,
    pub status_message: Option<String>,
    pub vector_count: usize,
    pub total_records: usize,
    pub vectorizing: bool,
    pub models: Vec<ModelInfo>,
    pub picker_active: bool,
    pub picker_target: ModelTarget,
    pub picker_search: String,
    pub picker_selected_idx: usize,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self::load()
    }
}

impl SettingsState {
    pub fn load() -> Self {
        let env_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
        let (stored_key, chat_model, embed_model, thresh) = load_config_file();
        let api_key = if !env_key.is_empty() {
            env_key
        } else {
            stored_key
        };

        let mut models = load_cached_models().unwrap_or_default();
        if models.is_empty() {
            if let Some(fetched) = fetch_openrouter_models() {
                models = fetched;
            }
        }

        let chat = if !chat_model.is_empty() {
            chat_model
        } else {
            "anthropic/claude-3.7-sonnet".to_string()
        };

        let embed = if !embed_model.is_empty() {
            embed_model
        } else {
            "openai/text-embedding-3-small".to_string()
        };

        Self {
            api_key,
            chat_model: chat,
            embedding_model: embed,
            similarity_threshold: thresh.clamp(30, 95),
            selected_field: SettingsField::ApiKey,
            editing_api_key: false,
            api_key_input: String::new(),
            status_message: None,
            vector_count: 0,
            total_records: 0,
            vectorizing: false,
            models,
            picker_active: false,
            picker_target: ModelTarget::Chat,
            picker_search: String::new(),
            picker_selected_idx: 0,
        }
    }

    pub fn refresh_models(&mut self) {
        if let Some(fetched) = fetch_openrouter_models() {
            let count = fetched.len();
            save_cached_models(&fetched);
            self.models = fetched;
            self.status_message = Some(format!("Načteno {} modelů z OpenRouteru", count));
        } else {
            self.status_message = Some("Nepodařilo se načíst modely z OpenRouteru".to_string());
        }
    }

    pub fn current_chat_info(&self) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.id == self.chat_model)
    }

    pub fn current_embedding_info(&self) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.id == self.embedding_model)
    }

    pub fn open_picker(&mut self, target: ModelTarget) {
        self.picker_active = true;
        self.picker_target = target;
        self.picker_search.clear();
        self.picker_selected_idx = 0;
    }

    pub fn close_picker(&mut self) {
        self.picker_active = false;
        self.picker_search.clear();
        self.picker_selected_idx = 0;
    }

    pub fn filtered_picker_models(&self) -> Vec<&ModelInfo> {
        filter_models(&self.models, &self.picker_search)
    }

    pub fn submit_picker(&mut self) {
        let (chosen_id, chosen_name) = {
            let filtered = self.filtered_picker_models();
            if let Some(chosen) = filtered.get(self.picker_selected_idx) {
                (Some(chosen.id.clone()), Some(chosen.name.clone()))
            } else if !self.picker_search.trim().is_empty() {
                let custom = self.picker_search.trim().to_string();
                (Some(custom.clone()), Some(custom))
            } else {
                (None, None)
            }
        };

        if let (Some(id), Some(name)) = (chosen_id, chosen_name) {
            match self.picker_target {
                ModelTarget::Chat => self.chat_model = id,
                ModelTarget::Embedding => self.embedding_model = id,
            }
            self.save();
            self.status_message = Some(format!("Vybrán model: {}", name));
        }
        self.close_picker();
    }

    pub fn picker_next(&mut self) {
        let count = self.filtered_picker_models().len();
        if count > 0 {
            self.picker_selected_idx = (self.picker_selected_idx + 1).min(count - 1);
        }
    }

    pub fn picker_prev(&mut self) {
        if self.picker_selected_idx > 0 {
            self.picker_selected_idx -= 1;
        }
    }

    pub fn next_chat_model(&mut self) {
        if self.models.is_empty() {
            return;
        }
        if let Some(idx) = self.models.iter().position(|m| m.id == self.chat_model) {
            let next_idx = (idx + 1) % self.models.len();
            self.chat_model = self.models[next_idx].id.clone();
            self.save();
        } else {
            self.chat_model = self.models[0].id.clone();
            self.save();
        }
    }

    pub fn prev_chat_model(&mut self) {
        if self.models.is_empty() {
            return;
        }
        if let Some(idx) = self.models.iter().position(|m| m.id == self.chat_model) {
            let prev_idx = (idx + self.models.len() - 1) % self.models.len();
            self.chat_model = self.models[prev_idx].id.clone();
            self.save();
        } else {
            self.chat_model = self.models[0].id.clone();
            self.save();
        }
    }

    pub fn next_embedding_model(&mut self) {
        if self.models.is_empty() {
            return;
        }
        if let Some(idx) = self.models.iter().position(|m| m.id == self.embedding_model) {
            let next_idx = (idx + 1) % self.models.len();
            self.embedding_model = self.models[next_idx].id.clone();
            self.save();
        } else {
            self.embedding_model = self.models[0].id.clone();
            self.save();
        }
    }

    pub fn prev_embedding_model(&mut self) {
        if self.models.is_empty() {
            return;
        }
        if let Some(idx) = self.models.iter().position(|m| m.id == self.embedding_model) {
            let prev_idx = (idx + self.models.len() - 1) % self.models.len();
            self.embedding_model = self.models[prev_idx].id.clone();
            self.save();
        } else {
            self.embedding_model = self.models[0].id.clone();
            self.save();
        }
    }

    pub fn increase_threshold(&mut self) {
        if self.similarity_threshold <= 90 {
            self.similarity_threshold += 5;
            self.save();
        }
    }

    pub fn decrease_threshold(&mut self) {
        if self.similarity_threshold >= 35 {
            self.similarity_threshold -= 5;
            self.save();
        }
    }

    pub fn start_editing_api_key(&mut self) {
        self.editing_api_key = true;
        self.api_key_input = self.api_key.clone();
    }

    pub fn cancel_editing_api_key(&mut self) {
        self.editing_api_key = false;
        self.api_key_input.clear();
    }

    pub fn submit_api_key(&mut self) {
        self.api_key = self.api_key_input.trim().to_string();
        self.editing_api_key = false;
        self.api_key_input.clear();
        self.save();
        self.status_message = Some("API klíč byl úspěšně uložen".to_string());
    }

    pub fn save(&self) {
        save_config_file(
            &self.api_key,
            &self.chat_model,
            &self.embedding_model,
            self.similarity_threshold,
        );
    }
}
