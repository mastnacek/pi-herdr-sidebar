//! State for the Shortcuts tab.
//!
//! Deliberately small: the tab only documents keybindings. Everything about
//! scanning pi's session logs lives in [`super::usage::state::UsageOverview`],
//! which belongs to the standalone overview window.
use super::keys::{self, ShortcutEntry, ShortcutSource};

#[derive(Debug)]
pub struct ShortcutsState {
    pub entries: Vec<ShortcutEntry>,
    /// Resolved Herdr prefix chord (usually `ctrl+b`).
    pub prefix: String,
    /// Where the user bindings came from, if a config was found.
    pub config_path: Option<String>,
    pub selected: usize,
    pub scroll: u16,
    pub status: Option<String>,
}

impl Default for ShortcutsState {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutsState {
    pub fn new() -> Self {
        let config = keys::load();
        Self {
            entries: config.entries,
            prefix: config.prefix,
            config_path: config.config_path.map(|p| p.to_string_lossy().to_string()),
            selected: 0,
            scroll: 0,
            status: None,
        }
    }

    /// Re-reads `config.toml` (bound to `r` on the Shortcuts tab).
    pub fn reload_keys(&mut self) {
        let config = keys::load();
        self.entries = config.entries;
        self.prefix = config.prefix;
        self.config_path = config.config_path.map(|p| p.to_string_lossy().to_string());
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        self.status = Some(format!("Načteno {} zkratek", self.entries.len()));
    }

    /// Chords the user bound themselves in `config.toml`.
    pub fn user_binding_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.source == ShortcutSource::User)
            .count()
    }

    pub fn selected_entry(&self) -> Option<&ShortcutEntry> {
        self.entries.get(self.selected)
    }

    pub fn next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1).min(self.entries.len() - 1);
        }
    }

    pub fn prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn scroll_by(&mut self, lines: i32) {
        let next = self.scroll as i32 + lines;
        self.scroll = next.clamp(0, u16::MAX as i32) as u16;
    }

    pub fn page_by(&mut self, down: bool) {
        self.scroll_by(if down { 8 } else { -8 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(count: usize) -> ShortcutsState {
        let mut state = ShortcutsState::new();
        state.entries = (0..count)
            .map(|i| ShortcutEntry {
                key: format!("ctrl+b+{i}"),
                kind: "default".to_string(),
                command: String::new(),
                description: format!("entry {i}"),
                source: ShortcutSource::Default,
            })
            .collect();
        state
    }

    #[test]
    fn selection_clamps_at_both_ends() {
        let mut state = state_with(3);
        state.prev();
        assert_eq!(state.selected, 0);
        state.next();
        state.next();
        state.next();
        assert_eq!(state.selected, 2, "cannot move past the last entry");
    }

    #[test]
    fn manual_scroll_is_clamped() {
        let mut state = state_with(2);
        state.scroll_by(-10);
        assert_eq!(state.scroll, 0, "cannot scroll above the first row");
        state.page_by(true);
        assert_eq!(state.scroll, 8);
        state.page_by(false);
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn reload_keeps_the_selection_inside_the_list() {
        let mut state = state_with(3);
        state.selected = 2;
        state.reload_keys();
        assert!(state.selected < state.entries.len());
        assert!(state.status.is_some());
    }

    #[test]
    fn loads_real_keybindings_from_a_config() {
        // Exercises the loader end-to-end against whatever is on this machine.
        let config = keys::load();
        assert!(!config.prefix.is_empty());
        assert!(
            !config.entries.is_empty(),
            "defaults must always be present"
        );
    }
}
