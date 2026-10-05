//! Model cycling and threshold adjustments for SettingsState.
use super::state::SettingsState;

impl SettingsState {
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
}
