// src/slices/spai_notes/actions.rs
use super::state::SpaiNotes;

#[derive(Debug, Clone)]
pub enum SpaiNotesAction {
    Add {
        title: String,
        body: String,
    },
    Edit {
        idx: usize,
        title: String,
        body: String,
    },
    Delete {
        idx: usize,
    },
    Select {
        idx: usize,
    },
    ToggleZen,
}

impl SpaiNotesAction {
    pub fn apply(self, model: &mut SpaiNotes) {
        match self {
            SpaiNotesAction::Add { title, body } => model.add(title, body),
            SpaiNotesAction::Edit { idx, title, body } => model.edit(idx, title, body),
            SpaiNotesAction::Delete { idx } => model.delete(idx),
            SpaiNotesAction::Select { idx } => model.select(idx),
            SpaiNotesAction::ToggleZen => model.toggle_zen(),
        }
    }
}
