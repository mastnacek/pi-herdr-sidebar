//! Creation dialog (`n`) key handling, including the on-demand dedup panel.
use crate::slices::view::state::{SidebarState, Tab};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Handles keys while the creation dialog is active. Returns `true` when consumed.
pub fn handle_creation_dialog(key: &KeyEvent, state: &mut SidebarState) -> bool {
    if state.active_tab != Tab::Notes || !state.spai_notes.creation_dialog.active {
        return false;
    }

    // The dedup overlay sits above the dialog and swallows its keys first.
    if state.spai_notes.dedup.visible {
        return handle_dedup_panel_keys(key, state);
    }

    let ac_active = state.spai_notes.creation_dialog.autocomplete_active;
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Char('d') | KeyCode::Char('D') if ctrl => {
            let threshold = state.settings.similarity_threshold as f64 / 100.0;
            state.spai_notes.run_dedup(
                &state.settings.api_key,
                &state.settings.embedding_model,
                threshold,
                state.spai_notes.creation_dialog.title_input.clone(),
            );
        }
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
        KeyCode::Left => state.spai_notes.on_dialog_cursor_left(),
        KeyCode::Right => state.spai_notes.on_dialog_cursor_right(),
        KeyCode::Home => state.spai_notes.on_dialog_cursor_home(),
        KeyCode::End => state.spai_notes.on_dialog_cursor_end(),
        KeyCode::Delete => state.spai_notes.on_dialog_delete(),
        KeyCode::Backspace => state.spai_notes.on_dialog_backspace(),
        KeyCode::Char(c) if crate::shared::keys::is_text_input(key) => {
            state.spai_notes.on_dialog_char_typed(c)
        }
        _ => {}
    }
    true
}

/// Ctrl+O/A/U over the visible dedup panel (creation dialog only).
fn handle_dedup_panel_keys(key: &KeyEvent, state: &mut SidebarState) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('o') | KeyCode::Char('O') if ctrl => {
            state.spai_notes.close_dedup_panel();
            if !state.spai_notes.open_first_similar_match() {
                state.spai_notes.close_dedup_panel();
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') if ctrl => {
            if let Err(e) = state.spai_notes.append_to_first_similar_match() {
                state.spai_notes.status_message = Some(e);
            }
        }
        KeyCode::Char('u') | KeyCode::Char('U') if ctrl => {
            if let Err(e) = state.spai_notes.cycle_status_of_first_similar_match() {
                state.spai_notes.status_message = Some(e);
            }
        }
        KeyCode::Esc => state.spai_notes.close_dedup_panel(),
        _ => {}
    }
    true
}
