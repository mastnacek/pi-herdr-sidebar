//! System-prompt provenance for the Status face.
//!
//! Pi persists the *system prompt itself* in the session transcript
//! (`pi-coding-agent/docs/session-format.md`): the first request writes a
//! `role: "system"` message whose `sections` field carries every prompt
//! section, and later changes arrive as further system messages that patch
//! sections by name (`null` removes one) and list `toolsAdded` /
//! `toolsRemoved`. Replaying them in order yields the exact prompt the model
//! received — so the sidebar can show which `AGENTS.md` files were loaded and
//! whether an `APPEND_SYSTEM.md` addendum made it into the request, without
//! re-discovering or guessing anything.
//!
//! Section order mirrors `dist/core/system-prompt.js#buildSystemPromptSections`:
//!
//! 1. `preamble`          base instructions, or `SYSTEM.md` / `--system-prompt`
//! 2. `tools`             one-line snippet per selected tool
//! 3. `rules`             tool + prompt guidelines
//! 4. `docs`              pointers into the pi documentation tree
//! 5. `addendum`          `APPEND_SYSTEM.md` (+ `--append-system-prompt`)
//! 6. `project_context`   every discovered `AGENTS.md` / `CLAUDE.md`
//! 7. `skills`            loaded skill descriptions
//! 8. `cwd`               resolved working directory
//!
//! Extension-supplied sections are appended after `cwd` by pi. `preamble` is
//! the only raw section; every other section reaches the model wrapped as
//! `<name>…</name>`.
//!
//! serde_json has no `preserve_order` feature here, so a JSON object iterates
//! alphabetically. The documented build order above is used instead, which is
//! also what the transcript's insertion order encodes.

mod replay;
mod sources;
mod text;

pub use replay::parse_prompt_tree;

/// Pi's section build order (see the module docs). Unknown names are
/// extension sections and render after these.
pub const SECTION_ORDER: [&str; 8] = [
    "preamble",
    "tools",
    "rules",
    "docs",
    "addendum",
    "project_context",
    "skills",
    "cwd",
];

/// Where a section's text came from, when it could be attributed to a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptSource {
    /// A file on disk whose content equals (or is a suffix of) the section.
    File(String),
    /// No file matched — inline `--system-prompt` / `--append-system-prompt`,
    /// a joined combination, or an extension.
    Inline,
}

/// Drift of a loaded source file against the prompt that carried its text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SourceState {
    /// Still identical to what the prompt shows.
    #[default]
    Ok,
    /// Changed on disk *after* the prompt was built — pi only re-reads on
    /// `/reload`, so the running prompt is stale.
    Modified,
    /// Was loaded but is gone from disk now.
    Missing,
}

/// One prompt section after replaying every patch.
#[derive(Debug, Clone, Default)]
pub struct PromptSection {
    pub name: String,
    /// Effective character count (`0` when the section was removed).
    pub chars: usize,
    /// First non-empty line, trimmed for the tree.
    pub preview: String,
    /// How many system messages wrote this section (`0` = one write).
    pub patches: u32,
    /// A later system message removed the section with `null`.
    pub removed: bool,
    /// Not part of pi's documented build order (extension-supplied).
    pub custom: bool,
}

/// An `<project_instructions path="…">` entry inside `project_context`.
#[derive(Debug, Clone, Default)]
pub struct PromptFileRef {
    pub path: String,
    pub chars: usize,
    /// Drift against the prompt that loaded it.
    pub state: SourceState,
}

/// Replayed system prompt of one session.
#[derive(Debug, Clone, Default)]
pub struct PromptTree {
    pub sections: Vec<PromptSection>,
    /// Every context file pi loaded, in the order it rendered them
    /// (agent directory first, then root → cwd).
    pub context_files: Vec<PromptFileRef>,
    /// `addendum` attribution (APPEND_SYSTEM.md), when the section exists,
    /// with the drift of the file it matched.
    pub append_system: Option<(PromptSource, SourceState)>,
    /// First line of the addendum body — names an inline (CLI) append even
    /// without the exact provider.
    pub addendum_preview: String,
    /// `SYSTEM.md` / `--system-prompt` attribution, when the default preamble
    /// was replaced.
    pub system_override: Option<(PromptSource, SourceState)>,
    /// First line of the replacement body.
    pub override_preview: String,
    /// Timestamp of the newest system message — the moment the effective
    /// prompt was assembled. Empty when the transcript has none.
    pub loaded_at: String,
    /// Effective tool loadout after `toolsAdded` / `toolsRemoved` replay.
    pub tools: Vec<String>,
    pub tools_added: u32,
    pub tools_removed: u32,
    /// System messages that declared a tool loadout (first request + patches).
    pub tool_messages: u32,
    /// System messages after the first one (each patches something).
    pub patches: u32,
    pub total_chars: usize,
}

/// Drift of a file loaded at `loaded_at` (an ISO-8601 timestamp).
///
/// Shared with the prompt sidecar, which knows the capture time from the engine
/// instead of from the transcript.
pub fn source_state(path: &str, loaded_at: &str) -> SourceState {
    sources::file_state(path, sources::iso_to_epoch_ms(loaded_at))
}

#[cfg(test)]
mod tests;
