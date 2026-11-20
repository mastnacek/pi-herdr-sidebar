//! Czech record body in the viewer. `spai show --json` returns `body_cs`; it is
//! asked in the background (one call at a time) for the selected record and kept
//! per file path + fingerprint (mtime, len), so a record is asked once until its
//! file changes. Until `spai` answers — or when it has no Czech body, is missing
//! or fails — the viewer shows the English body of the file. Editing never
//! touches this: `e`/`E` always work on the English file.
use super::discovery::file_fingerprint;
use super::note::SpaiNoteItem;
use super::state::SpaiNotesState;
use crate::shared::spai_client;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

type Fingerprint = (u64, u64);
type Answer = Result<Option<String>, String>;

#[derive(Debug, Default)]
pub struct BodyCs {
    /// `t` switched the viewer to the English body.
    pub show_en: bool,
    /// Per file path: the fingerprint it was asked at and the Czech body (if any).
    cache: HashMap<PathBuf, (Fingerprint, Option<String>)>,
    pending: Option<Pending>,
}

#[derive(Debug)]
struct Pending {
    path: PathBuf,
    fp: Fingerprint,
    rx: Receiver<Answer>,
}

impl BodyCs {
    #[cfg(test)]
    fn pending_is_some(&self) -> bool {
        self.pending.is_some()
    }
}

impl SpaiNotesState {
    /// Tick: takes a finished answer, then asks for the selected record when it
    /// is not cached at its current fingerprint. A failure is cached as "no
    /// Czech body", so a missing `spai` is not started again every tick.
    pub fn poll_body_cs(&mut self) {
        if let Some(p) = &self.body_cs.pending {
            match p.rx.try_recv() {
                Ok(answer) => {
                    let p = self.body_cs.pending.take().expect("pending");
                    self.body_cs.cache.insert(p.path, (p.fp, answer.unwrap_or(None)));
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => self.body_cs.pending = None,
            }
        }
        let (Some(item), Some(project)) = (self.selected_item(), self.selected_project_path())
        else {
            return;
        };
        let Some(fp) = file_fingerprint(&item.file_path) else {
            return;
        };
        let fresh = self.body_cs.cache.get(&item.file_path).is_some_and(|(f, _)| *f == fp);
        if fresh {
            return;
        }
        let (path, id, cfg) = (item.file_path.clone(), item.id.clone(), self.core.clone());
        let rx = spai_client::spawn(move || spai_client::show_body_cs(&cfg, &project, &id));
        self.body_cs.pending = Some(Pending { path, fp, rx });
    }

    /// `t`: Czech <-> English body in the viewer.
    pub fn toggle_body_lang(&mut self) {
        self.body_cs.show_en = !self.body_cs.show_en;
        self.viewer_scroll = 0;
        self.status_message = Some(
            if self.body_cs.show_en { "Tělo: angličtina (t = česky)" } else { "Tělo: česky (t = angličtina)" }
                .to_string(),
        );
    }

    /// The body the viewer shows for `item`: Czech when `spai` has it and the
    /// user did not switch to English, else the English body of the file.
    pub fn viewer_body<'a>(&'a self, item: &'a SpaiNoteItem) -> &'a str {
        if !self.body_cs.show_en {
            if let Some((_, Some(cs))) = self.body_cs.cache.get(&item.file_path) {
                return cs;
            }
        }
        &item.body
    }
}

#[cfg(test)]
#[path = "state_body_cs_tests.rs"]
mod tests;
