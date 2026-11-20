//! Type-to-search over the project list of the Notes tab (`/`).
//!
//! While the line is open it owns the keyboard: printable keys edit the query,
//! arrows move among the matches, Enter keeps the choice, Esc goes back to the
//! project selected before.
use super::scratch::filter::normalize_czech;
use super::state::SpaiNotesState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ProjectFilter {
    pub query: String,
    /// The project selected when the line was opened (Esc returns to it).
    before: Option<PathBuf>,
}

impl SpaiNotesState {
    pub fn open_project_filter(&mut self) {
        self.project_filter = Some(ProjectFilter {
            query: String::new(),
            before: self.selected_project_path(),
        });
    }

    /// Indexes into `projects` whose name or path contains the query
    /// (case and diacritics ignored). An empty query matches everything.
    pub fn project_filter_matches(&self) -> Vec<usize> {
        let Some(filter) = &self.project_filter else {
            return (0..self.projects.len()).collect();
        };
        let needle = normalize_czech(filter.query.trim());
        (0..self.projects.len())
            .filter(|&i| {
                let p = &self.projects[i];
                let hay = format!("{} {}", p.name, p.path.to_string_lossy().replace('\\', "/"));
                normalize_czech(&hay).contains(&needle)
            })
            .collect()
    }

    /// Keeps the selection on a match after the query changed; no match keeps it as is.
    fn follow_project_filter(&mut self) {
        let matches = self.project_filter_matches();
        if let Some(&first) = matches.first() {
            if !matches.contains(&self.selected_project_idx) {
                self.select_project(first);
            }
        }
    }

    /// Handles a key while the filter line is open. Returns `false` when it is closed;
    /// when open every key is consumed so none reaches the global shortcuts.
    pub fn handle_project_filter_key(&mut self, key: &KeyEvent) -> bool {
        if self.project_filter.is_none() {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                let before = self.project_filter.take().and_then(|f| f.before);
                self.reselect_project(before);
            }
            KeyCode::Enter => self.project_filter = None,
            KeyCode::Backspace => {
                if let Some(f) = &mut self.project_filter {
                    f.query.pop();
                }
                self.follow_project_filter();
            }
            KeyCode::Left | KeyCode::Up => self.prev_project(),
            KeyCode::Right | KeyCode::Down => self.next_project(),
            KeyCode::Char(c)
                if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(f) = &mut self.project_filter {
                    f.query.push(c);
                }
                self.follow_project_filter();
            }
            _ => {}
        }
        true
    }
}
