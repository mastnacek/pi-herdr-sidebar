//! `CoreConfig`: the runner a slice holds to call `spai`.
use super::runner::{find_binary, ProcessRunner, Runner};
use std::sync::Arc;

#[derive(Clone)]
pub struct CoreConfig {
    pub runner: Arc<dyn Runner>,
}

impl CoreConfig {
    pub fn from_process() -> Self {
        Self {
            runner: Arc::new(ProcessRunner {
                bin: find_binary(),
                env: Vec::new(),
            }),
        }
    }
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self::from_process()
    }
}

impl std::fmt::Debug for CoreConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CoreConfig")
    }
}
