//! State and models for S.P.A.I. & OpenRouter Vectorization settings.
use std::path::PathBuf;

pub const CHAT_MODELS: &[&str] = &[
    "anthropic/claude-3.7-sonnet",
    "openai/gpt-4o-mini",
    "google/gemini-2.5-flash",
    "deepseek/deepseek-chat",
    "qwen/qwen-2.5-coder-32b-instruct",
    "meta-llama/llama-3.3-70b-instruct",
];

pub const EMBEDDING_MODELS: &[&str] = &[
    "openai/text-embedding-3-small",
    "openai/text-embedding-3-large",
    "baai/bge-m3",
    "qwen/qwen2.5-7b-instruct",
];

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
    pub const ALL: &[SettingsField] = &[
        SettingsField::ApiKey,
        SettingsField::ChatModel,
        SettingsField::EmbeddingModel,
        SettingsField::SimilarityThreshold,
        SettingsField::VectorizeAction,
        SettingsField::ClassifyAction,
    ];

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

#[derive(Debug, Clone)]
pub struct SettingsState {
    pub api_key: String,
    pub chat_model_idx: usize,
    pub embedding_model_idx: usize,
    pub similarity_threshold: u8,
    pub selected_field: SettingsField,
    pub editing_api_key: bool,
    pub api_key_input: String,
    pub status_message: Option<String>,
    pub vector_count: usize,
    pub total_records: usize,
    pub vectorizing: bool,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self::load()
    }
}

impl SettingsState {
    pub fn load() -> Self {
        let env_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
        let (stored_key, chat_idx, embed_idx, thresh) = Self::load_config_file();
        let api_key = if !env_key.is_empty() {
            env_key
        } else {
            stored_key
        };

        Self {
            api_key,
            chat_model_idx: chat_idx.min(CHAT_MODELS.len().saturating_sub(1)),
            embedding_model_idx: embed_idx.min(EMBEDDING_MODELS.len().saturating_sub(1)),
            similarity_threshold: thresh.clamp(30, 95),
            selected_field: SettingsField::ApiKey,
            editing_api_key: false,
            api_key_input: String::new(),
            status_message: None,
            vector_count: 0,
            total_records: 0,
            vectorizing: false,
        }
    }

    pub fn current_chat_model(&self) -> &'static str {
        CHAT_MODELS[self.chat_model_idx % CHAT_MODELS.len()]
    }

    pub fn current_embedding_model(&self) -> &'static str {
        EMBEDDING_MODELS[self.embedding_model_idx % EMBEDDING_MODELS.len()]
    }

    pub fn next_chat_model(&mut self) {
        self.chat_model_idx = (self.chat_model_idx + 1) % CHAT_MODELS.len();
        self.save();
    }

    pub fn prev_chat_model(&mut self) {
        self.chat_model_idx = (self.chat_model_idx + CHAT_MODELS.len() - 1) % CHAT_MODELS.len();
        self.save();
    }

    pub fn next_embedding_model(&mut self) {
        self.embedding_model_idx = (self.embedding_model_idx + 1) % EMBEDDING_MODELS.len();
        self.save();
    }

    pub fn prev_embedding_model(&mut self) {
        self.embedding_model_idx =
            (self.embedding_model_idx + EMBEDDING_MODELS.len() - 1) % EMBEDDING_MODELS.len();
        self.save();
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

    fn config_path() -> Option<PathBuf> {
        if let Ok(dir) = std::env::var("HERDR_PLUGIN_CONFIG_DIR") {
            let p = PathBuf::from(dir).join("spai-settings.json");
            return Some(p);
        }
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .ok()
            .map(|h| PathBuf::from(h).join(".pi").join("agent").join("spai-settings.json"))
    }

    fn load_config_file() -> (String, usize, usize, u8) {
        let Some(path) = Self::config_path() else {
            return (String::new(), 0, 0, 55);
        };
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                let key = val["api_key"].as_str().unwrap_or_default().to_string();
                let chat = val["chat_model_idx"].as_u64().unwrap_or(0) as usize;
                let embed = val["embedding_model_idx"].as_u64().unwrap_or(0) as usize;
                let thresh = val["similarity_threshold"].as_u64().unwrap_or(55) as u8;
                return (key, chat, embed, thresh);
            }
        }
        (String::new(), 0, 0, 55)
    }

    pub fn save(&self) {
        let Some(path) = Self::config_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::json!({
            "api_key": self.api_key,
            "chat_model_idx": self.chat_model_idx,
            "embedding_model_idx": self.embedding_model_idx,
            "similarity_threshold": self.similarity_threshold,
        });
        let _ = std::fs::write(path, json.to_string());
    }
}
