//! Integrated editor key handling, branched by EditMode (plan §3).
use super::super::external::open_external_editor;
use crate::slices::view::state::{SidebarState, Tab};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Handles keys while the editor is active. Returns `true` when consumed.
pub fn handle_edit_dialog(
    key: &KeyEvent,
    state: &mut SidebarState,
    guard: &mut crate::shared::TerminalGuard,
) -> bool {
    if state.active_tab != Tab::Notes || !state.spai_notes.edit_dialog.active {
        return false;
    }

    // The dedup overlay sits above the editor: Ctrl+D re-runs, Esc closes.
    if state.spai_notes.dedup.visible {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('d') | KeyCode::Char('D') if ctrl => run_editor_dedup(state),
            KeyCode::Esc => state.spai_notes.close_dedup_panel(),
            _ => {}
        }
        return true;
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    // Mode-independent keys first.
    match key.code {
        KeyCode::Char('s') if ctrl => {
            if let Err(err) = state.spai_notes.submit_edit_dialog() {
                state.spai_notes.status_message = Some(err);
            }
            return true;
        }
        KeyCode::Char('e') | KeyCode::Char('E') if ctrl => {
            state.spai_notes.close_edit_dialog();
            open_external_editor(guard, state);
            return true;
        }
        KeyCode::Tab | KeyCode::BackTab => {
            state.spai_notes.toggle_edit_field();
            return true;
        }
        _ => {}
    }

    if state.spai_notes.edit_dialog.mode == crate::slices::spai_notes::dialog_state::EditMode::Insert
    {
        handle_insert_keys(key, state);
    } else {
        handle_normal_keys(key, state);
    }
    true
}

fn run_editor_dedup(state: &mut SidebarState) {
    let threshold = state.settings.similarity_threshold as f64 / 100.0;
    state.spai_notes.run_dedup(
        &state.settings.api_key,
        &state.settings.embedding_model,
        threshold,
        state.spai_notes.edit_dialog.title_input.clone(),
    );
}

fn handle_insert_keys(key: &KeyEvent, state: &mut SidebarState) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => state.spai_notes.leave_insert_mode(),
        KeyCode::Enter => state.spai_notes.on_edit_enter(),
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
}

fn handle_normal_keys(key: &KeyEvent, state: &mut SidebarState) {
    use crate::slices::spai_notes::state_edit::At;
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.spai_notes.request_close_edit_dialog();
        }
        KeyCode::Char('i') => state.spai_notes.enter_insert_mode(At::Cursor),
        KeyCode::Char('a') => state.spai_notes.enter_insert_mode(At::AfterCursor),
        KeyCode::Char('A') => state.spai_notes.enter_insert_mode(At::LineEnd),
        KeyCode::Char('x') => {
            let _ = state.spai_notes.cycle_selected_status();
        }
        KeyCode::Char('d') | KeyCode::Delete => state.spai_notes.begin_delete_selected(),
        KeyCode::Up | KeyCode::Char('k') => state.spai_notes.on_edit_cursor_vertical(false),
        KeyCode::Down | KeyCode::Char('j') => state.spai_notes.on_edit_cursor_vertical(true),
        KeyCode::PageUp => state.spai_notes.scroll_edit_body(false, 5),
        KeyCode::PageDown => state.spai_notes.scroll_edit_body(true, 5),
        KeyCode::Char('g') | KeyCode::Char('G') => {
            state.spai_notes.on_normal_key(key.code);
        }
        _ => {}
    }
}
