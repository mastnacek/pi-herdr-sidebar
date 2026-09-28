//! `<pane>.prompt.json` — the exact system prompt the engine resolved.
//!
//! Published by the TypeScript `pi-sidebar` extension from Pi's
//! `before_agent_start` event, where the engine hands over the fully resolved
//! `systemPromptOptions` it renders the request from. That is the only place
//! the *exact* provenance of a prompt is visible:
//!
//! * `forceSystemPrompt` / `customPrompt` — a `SYSTEM.md` override or an
//!   extension forcing the whole prompt;
//! * an `appendSystemPrompt` that is inline, CLI-supplied or joined from
//!   several sources and therefore matches no single file;
//! * context files with their real paths;
//! * the section set and per-section character counts the model received.
//!
//! The transcript replay in [`super::prompt_tree`] remains the fallback: it
//! needs no extension, but it has to infer section sources and cannot see a
//! forced prompt. Both are rendered by the same tree; this module only parses
//! the sidecar and reports drift of the files it names.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::prompt_tree::{source_state, PromptSource, SourceState};

/// Highest sidecar shape this reader understands. A newer file is ignored
/// rather than mis-rendered, and the face falls back to the transcript.
pub const SIDECAR_VERSION: u32 = 1;

/// One block of prompt text as the engine reported it.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PromptTextSource {
    #[serde(default)]
    pub chars: usize,
    /// `"file"` or `"inline"`.
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub path: Option<String>,
    /// First non-empty line of the text, trimmed and truncated. The engine hands
    /// extensions only the text, so an inline append (`--append-system-prompt`,
    /// joined sources) would otherwise be anonymous — the preview names it.
    #[serde(default)]
    pub preview: String,
}

impl PromptTextSource {
    pub fn is_file(&self) -> bool {
        self.source == "file" && self.path.is_some()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PromptContextFile {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub chars: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PromptSkillRef {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "descriptionChars")]
    pub description_chars: usize,
}

/// Parsed `.prompt.json` sidecar.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PromptSidecar {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub live: bool,
    #[serde(default, rename = "capturedAt")]
    pub captured_at: String,
    #[serde(default)]
    pub forced: bool,
    #[serde(default, rename = "customPrompt")]
    pub custom_prompt: Option<PromptTextSource>,
    #[serde(default, rename = "appendSystemPrompt")]
    pub append_system_prompt: Option<PromptTextSource>,
    #[serde(default)]
    pub sections: Vec<String>,
    #[serde(default, rename = "sectionChars")]
    pub section_chars: BTreeMap<String, usize>,
    #[serde(default, rename = "contextFiles")]
    pub context_files: Vec<PromptContextFile>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub skills: Vec<PromptSkillRef>,
    #[serde(default, rename = "systemPromptChars")]
    pub system_prompt_chars: usize,
}

impl PromptSidecar {
    /// Parse a sidecar file. A dead sidecar (session over) reads as `None`, so
    /// the face falls back to the transcript instead of showing stale state.
    pub fn read_from_file(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        let sidecar: Self = serde_json::from_str(&content).ok()?;
        if !sidecar.live || sidecar.version > SIDECAR_VERSION {
            return None;
        }
        Some(sidecar)
    }

    /// Character count of a section, as the engine reported it.
    pub fn section_chars(&self, name: &str) -> Option<usize> {
        if !self.sections.iter().any(|s| s == name) {
            return None;
        }
        Some(self.section_chars.get(name).copied().unwrap_or(0))
    }

    /// Drift of a file the engine loaded at capture time.
    pub fn file_state(&self, path: &str) -> SourceState {
        source_state(path, &self.captured_at)
    }

    /// Attribution of a text block, with drift when it resolved to a file.
    pub fn attribute(
        &self,
        text: &Option<PromptTextSource>,
    ) -> Option<(PromptSource, SourceState)> {
        let text = text.as_ref()?;
        if text.is_file() {
            let path = text.path.clone().unwrap_or_default();
            let state = self.file_state(&path);
            Some((PromptSource::File(path), state))
        } else {
            Some((PromptSource::Inline, SourceState::Ok))
        }
    }
}

/// Sidecar path for a pane snapshot path: `<snapshot>.json` → `<snapshot>.prompt.json`.
pub fn sidecar_path(snapshot_path: &Path) -> PathBuf {
    let mut path = snapshot_path.to_path_buf();
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("sidebar")
        .to_string();
    path.set_file_name(format!("{name}.prompt.json"));
    path
}

#[cfg(test)]
mod tests;
