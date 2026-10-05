//! Dialog key dispatch composition for SPAI notes and settings.
//!
//! Split per the plan: creation (`n`) lives in [`super::creation`], the
//! integrated editor with Normal/Insert modes in [`super::edit`]; this file
//! only orders the handlers and keeps the settings dialogs.


use super::super::state::{SidebarState, Tab};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_settings_dialogs(key: &KeyEvent, state: &mut SidebarState) -> bool {
    if state.active_tab != Tab::Settings {
        return false;
    }

    if state.settings.picker_active {
        match key.code {
            KeyCode::Esc => state.settings.close_picker(),
            KeyCode::Enter => state.settings.submit_picker(),
            KeyCode::Up => state.settings.picker_prev(),
            KeyCode::Down => state.settings.picker_next(),
            KeyCode::PageUp => {
                for _ in 0..5 {
                    state.settings.picker_prev();
                }
            }
            KeyCode::PageDown => {
                for _ in 0..5 {
                    state.settings.picker_next();
                }
            }
            KeyCode::Backspace => {
                state.settings.picker_search.pop();
                state.settings.picker_selected_idx = 0;
            }
            KeyCode::Char('r') if state.settings.picker_search.is_empty() => {
                state.settings.refresh_models();
            }
            KeyCode::Char(c) => {
                state.settings.picker_search.push(c);
                state.settings.picker_selected_idx = 0;
            }
            _ => {}
        }
        return true;
    }

    if state.settings.editing_api_key {
        match key.code {
            KeyCode::Esc => state.settings.cancel_editing_api_key(),
            KeyCode::Enter => state.settings.submit_api_key(),
            KeyCode::Backspace => {
                state.settings.api_key_input.pop();
            }
            KeyCode::Char(c) => {
                state.settings.api_key_input.push(c);
            }
            _ => {}
        }
        return true;
    }

    false
}

pub fn handle_notes_dialogs(
    key: &KeyEvent,
    state: &mut SidebarState,
    guard: &mut crate::shared::TerminalGuard,
) -> bool {
    if state.active_tab != Tab::Notes {
        return false;
    }

    // Two-step delete confirmation swallows every key until resolved.
    if state.spai_notes.delete_confirm_active {
        match key.code {
            KeyCode::Delete => {
                if let Err(e) = state.spai_notes.confirm_delete_selected() {
                    state.spai_notes.status_message = Some(e);
                }
                // A deleted record cannot stay in the editor.
                if state.spai_notes.edit_dialog.active {
                    state.spai_notes.close_edit_dialog();
                }
            }
            _ => state.spai_notes.cancel_delete(),
        }
        return true;
    }

    super::edit::handle_edit_dialog(key, state, guard) || super::creation::handle_creation_dialog(key, state)
}
