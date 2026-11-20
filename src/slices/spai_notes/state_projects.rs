//! The project list from `spai projects`. The old discovery (cwd + pi cache)
//! shows a list at once; `spai` answers in the background and its list replaces
//! it. When `spai` is missing or fails the old list simply stays.
use super::discovery::{find_spai_dir, SpaiProjectSummary};
use super::state::SpaiNotesState;
use crate::shared::spai_client::{self, ListedProject};
use std::sync::mpsc::TryRecvError;

impl SpaiNotesState {
    /// Asks `spai projects` in the background (one call at a time).
    pub(super) fn request_project_list(&mut self) {
        if self.projects_rx.is_some() {
            return;
        }
        let Some(cwd) = self
            .current_project_path
            .clone()
            .or_else(|| std::env::current_dir().ok())
        else {
            return;
        };
        let cfg = self.core.clone();
        self.projects_rx = Some(spai_client::spawn(move || {
            spai_client::project_list(&cfg, &cwd)
        }));
    }

    /// Takes a finished answer; an error keeps the current list. Passive (tick).
    pub fn poll_projects(&mut self) {
        let Some(rx) = &self.projects_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => {
                self.projects_rx = None;
                if let Ok(listed) = result {
                    self.apply_project_list(listed);
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => self.projects_rx = None,
        }
    }

    /// Swaps in the list of `spai` (its order). Projects already known keep their
    /// loaded notes; the selected project and note stay where they were. An empty
    /// list is not believed: the fallback stays.
    pub(super) fn apply_project_list(&mut self, listed: Vec<ListedProject>) {
        let selected = self.selected_project_path();
        let mut old = std::mem::take(&mut self.projects);
        for item in listed {
            let Some(spai_dir) = find_spai_dir(&item.path) else {
                continue;
            };
            let mut summary = match old.iter().position(|p| p.path == item.path) {
                Some(at) => old.swap_remove(at),
                None => SpaiProjectSummary::new(item.name.clone(), item.path.clone(), spai_dir),
            };
            summary.name = item.name;
            self.projects.push(summary);
        }
        if self.projects.is_empty() {
            self.projects = old;
            return;
        }
        self.spai_listed = true;
        self.reselect_project(selected);
        self.notes_fingerprint = self.notes_fingerprint_now();
    }
}
