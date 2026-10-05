//! Dialog key event dispatch for SPAI notes and settings.
use crate::shared::TerminalGuard;
use crate::slices::view::external::open_external_editor;
use crate::slices::view::state::{SidebarState, Tab};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    guard: &mut TerminalGuard,
) -> bool {
    if state.active_tab != Tab::Notes {
        return false;
    }

    if state.spai_notes.creation_dialog.active {
        let ac_active = state.spai_notes.creation_dialog.autocomplete_active;
        match key.code {
            KeyCode::Esc => {
                if ac_active {
                    state.spai_notes.creation_dialog.autocomplete_active = false;
                } else {
                    state.spai_notes.close_creation_dialog();
                }
            }
            KeyCode::Up => {
                if ac_active {
                    state.spai_notes.prev_suggestion();
                } else {
                    state.spai_notes.prev_type();
                }
            }
            KeyCode::Down => {
                if ac_active {
                    state.spai_notes.next_suggestion();
                } else {
                    state.spai_notes.next_type();
                }
            }
            KeyCode::Tab => {
                if ac_active {
                    state.spai_notes.apply_selected_suggestion();
                } else {
                    state.spai_notes.apply_selected_type();
                }
            }
            KeyCode::Enter => {
                if ac_active {
                    state.spai_notes.apply_selected_suggestion();
                } else {
                    let _ = state.spai_notes.submit_creation_dialog();
                }
            }
            KeyCode::Backspace => state.spai_notes.on_dialog_backspace(),
            KeyCode::Char(c) => state.spai_notes.on_dialog_char_typed(c),
            _ => {}
        }
        return true;
    }

    if state.spai_notes.edit_dialog.active {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => state.spai_notes.close_edit_dialog(),
            KeyCode::Char('s') if ctrl => {
                if let Err(err) = state.spai_notes.submit_edit_dialog() {
                    state.spai_notes.status_message = Some(err);
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') if ctrl => {
                state.spai_notes.close_edit_dialog();
                open_external_editor(guard, state);
            }
            KeyCode::Enter => state.spai_notes.on_edit_enter(),
            KeyCode::Tab | KeyCode::BackTab => state.spai_notes.toggle_edit_field(),
            KeyCode::Backspace => state.spai_notes.on_edit_backspace(),
            KeyCode::Delete => state.spai_notes.on_edit_delete(),
            KeyCode::Left => state.spai_notes.on_edit_cursor_left(),
            KeyCode::Right => state.spai_notes.on_edit_cursor_right(),
            KeyCode::Up => state.spai_notes.on_edit_cursor_vertical(false),
            KeyCode::Down => state.spai_notes.on_edit_cursor_vertical(true),
            KeyCode::Home => state.spai_notes.on_edit_cursor_line_start(),
            KeyCode::End => state.spai_notes.on_edit_cursor_line_end(),
            KeyCode::PageUp => state.spai_notes.scroll_edit_body(false, 5),
            KeyCode::PageDown => state.spai_notes.scroll_edit_body(true, 5),
            KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => {
                state.spai_notes.on_edit_char_typed(c)
            }
            _ => {}
        }
        return true;
    }

    false
}
