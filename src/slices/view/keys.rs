//! Keyboard dispatch for the sidebar event loop.
//!
//! Split out of `run_view` so the loop stays readable and each face's key
//! handling sits in one place. The order of the blocks below is meaningful:
//! active modals consume keys *before* the global quit check, and `Esc` always
//! closes a modal rather than exiting the sidebar.
use super::external::open_external_editor;
use super::state::{SidebarState, Tab};
use crate::shared::TerminalGuard;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Handles one key press. Returns `true` when the sidebar should quit.
pub fn handle_key(key: KeyEvent, state: &mut SidebarState, guard: &mut TerminalGuard) -> bool {
    // Windows terminals emit Press AND Release events; react to presses only,
    // otherwise 'w' cycles two weather locations per keystroke.
    if key.kind != KeyEventKind::Press {
        return false;
    }

    if handle_notes_dialogs(&key, state, guard) {
        return false;
    }

    // Quit on q or Esc, or Ctrl+c
    if key.code == KeyCode::Char('q')
        || key.code == KeyCode::Esc
        || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
    {
        return true;
    }

    handle_global_keys(&key, state, guard);
    false
}

/// Creation dialog + integrated note editor. Returns `true` when the key was
/// consumed by a dialog.
fn handle_notes_dialogs(
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
            KeyCode::Up if ac_active => state.spai_notes.prev_suggestion(),
            KeyCode::Down if ac_active => state.spai_notes.next_suggestion(),
            KeyCode::Tab => {
                if ac_active {
                    state.spai_notes.apply_selected_suggestion();
                } else {
                    state.spai_notes.cycle_creation_kind();
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
            // Ctrl+E — never a bare `E`. Inside a text field every printable key
            // must insert its character: typing "Editace" used to spawn the
            // external editor instead of typing.
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

fn handle_global_keys(key: &KeyEvent, state: &mut SidebarState, guard: &mut TerminalGuard) {
    match key.code {
        KeyCode::Tab => state.next_tab(),
        KeyCode::BackTab => state.prev_tab(),
        KeyCode::Left | KeyCode::Char('h') => {
            if state.active_tab == Tab::Notes {
                state.spai_notes.prev_project();
            } else {
                state.prev_tab();
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if state.active_tab == Tab::Notes {
                state.spai_notes.next_project();
            } else {
                state.next_tab();
            }
        }
        KeyCode::Char('0') => state.set_tab(Tab::Zen),
        KeyCode::Char('1') => state.set_tab(Tab::Status),
        KeyCode::Char('2') => state.set_tab(Tab::Skills),
        KeyCode::Char('3') => state.set_tab(Tab::Mcp),
        KeyCode::Char('4') => state.set_tab(Tab::Notes),
        KeyCode::Char('5') => state.set_tab(Tab::Shortcuts),
        KeyCode::Char('p') if state.active_tab == Tab::Notes => {
            state.spai_notes.jump_to_active_project();
        }
        KeyCode::Char(',') | KeyCode::Char('<') | KeyCode::Char('[')
            if state.active_tab == Tab::Notes =>
        {
            state.spai_notes.prev_project();
        }
        KeyCode::Char('.') | KeyCode::Char('>') | KeyCode::Char(']')
            if state.active_tab == Tab::Notes =>
        {
            state.spai_notes.next_project();
        }
        KeyCode::Char('n') if state.active_tab == Tab::Notes => {
            state.spai_notes.open_creation_dialog();
        }
        KeyCode::Char('e')
            if state.active_tab == Tab::Notes && key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            state.spai_notes.close_edit_dialog();
            open_external_editor(guard, state);
        }
        KeyCode::Char('E') if state.active_tab == Tab::Notes => {
            state.spai_notes.close_edit_dialog();
            open_external_editor(guard, state);
        }
        KeyCode::Char('e') if state.active_tab == Tab::Notes => state.spai_notes.open_edit_dialog(),
        KeyCode::Char('x') if state.active_tab == Tab::Notes => {
            let _ = state.spai_notes.cycle_selected_status();
        }
        KeyCode::Up | KeyCode::Char('k') => match state.active_tab {
            Tab::Notes => state.spai_notes.prev_item(),
            Tab::Shortcuts => state.shortcuts.prev(),
            _ => state.scroll_up(1),
        },
        KeyCode::Down | KeyCode::Char('j') => match state.active_tab {
            Tab::Notes => state.spai_notes.next_item(),
            Tab::Shortcuts => state.shortcuts.next(),
            _ => state.scroll_down(1),
        },
        KeyCode::PageUp => match state.active_tab {
            Tab::Notes => state.spai_notes.scroll_viewer_up(6),
            Tab::Shortcuts => state.shortcuts.page_by(false),
            _ => state.scroll_up(10),
        },
        KeyCode::PageDown => match state.active_tab {
            Tab::Notes => state.spai_notes.scroll_viewer_down(6),
            Tab::Shortcuts => state.shortcuts.page_by(true),
            _ => state.scroll_down(10),
        },
        KeyCode::Char('d') if state.active_tab == Tab::Notes => {
            state.spai_notes.scroll_viewer_down(4);
        }
        KeyCode::Char('u') if state.active_tab == Tab::Notes => {
            state.spai_notes.scroll_viewer_up(4);
        }
        KeyCode::Home => state.scroll = 0,
        KeyCode::Char('r') => {
            if state.active_tab == Tab::Shortcuts {
                state.shortcuts.reload_keys();
            } else {
                state.trigger_manual_refresh();
            }
        }
        KeyCode::Char('w') => state.cycle_weather_location(),
        KeyCode::Char('c') => state.copy_weather_report(),
        _ => {}
    }
}
