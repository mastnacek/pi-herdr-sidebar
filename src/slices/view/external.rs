//! Launching the user's external editor on the selected SPAI note.
//!
//! Split out of the event loop: the TUI has to be suspended and resumed around
//! the child process, and any failure is reported in the status line rather than
//! propagated — suspending the terminal must never take the sidebar down.
use super::state::SidebarState;
use crate::shared::TerminalGuard;

pub fn open_external_editor(guard: &mut TerminalGuard, state: &mut SidebarState) {
    use crate::slices::spai_notes::external_editor;

    let Some(path) = state.spai_notes.selected_file_path() else {
        state.spai_notes.status_message = Some("Není vybrána žádná poznámka".to_string());
        return;
    };

    let Some(editor) = external_editor::resolve_editor() else {
        state.spai_notes.status_message =
            Some("Není nastaven žádný editor (proměnná $VISUAL nebo $EDITOR)".to_string());
        return;
    };

    if let Err(err) = guard.suspend() {
        state.spai_notes.status_message = Some(format!("Terminál nelze pozastavit: {err}"));
        return;
    }

    let result = external_editor::run_editor(&editor, &path);

    if let Err(err) = guard.resume() {
        state.spai_notes.status_message = Some(format!("Terminál nelze obnovit: {err}"));
        return;
    }

    match result {
        Ok(status) if status.success() => {
            state.spai_notes.refresh(None);
            state.spai_notes.status_message =
                Some(format!("Externí editor ({}) dokončen", editor.display()));
        }
        Ok(status) => {
            state.spai_notes.status_message =
                Some(format!("Externí editor skončil s chybou: {status}"));
        }
        Err(err) => {
            state.spai_notes.status_message =
                Some(format!("Editor '{}' nelze spustit: {err}", editor.program));
        }
    }
}
