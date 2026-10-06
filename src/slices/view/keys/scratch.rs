//! Scratchpad key dispatch — **one mode, one keymap**.
//!
//! The Scratchpad is a single-surface editor: everything is typeable
//! (bidirectional saving) and every extra action lives behind a Ctrl combo,
//! so plain letters always type. Opened by the global `Ctrl+N`; consumes
//! every key while open.
//!
//! - `Ctrl+S` save · `Ctrl+D` dedup · `Ctrl+O` open record · `Ctrl+X` status
//! - `Ctrl+Z` undo batch · `Ctrl+T` scope · `Ctrl+F` filter · `Ctrl+R` semantic
//! - `Ctrl+Y` status filter · `Ctrl+L` clear filters · `F1` help
//! - `Esc` close (twice when unsaved; the draft is kept regardless)
use super::super::state::SidebarState;
use crate::slices::spai_notes::scratch::filter::StatusFilter;
use crate::slices::spai_notes::scratch::state::{ScratchInput, ScratchScope};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Opens the Scratchpad (Ctrl+N), closing any dialog that owns the keys.
pub fn open_scratch(state: &mut SidebarState) {
    state.spai_notes.creation_dialog.active = false;
    state.spai_notes.close_edit_dialog();
    state.spai_notes.open_scratch(state.state_dir.clone());
}

/// Handles keys while the Scratchpad is open. Returns `true` when consumed.
pub fn handle_scratch_key(key: &KeyEvent, state: &mut SidebarState) -> bool {
    // The Scratchpad is a global fullscreen modal (Ctrl+N from any tab), so
    // the active tab does not gate it — only its own open flag does.
    if !state.spai_notes.scratch.open {
        return false;
    }

    // The F1 help overlay owns the keyboard while visible: any key closes.
    if state.spai_notes.scratch.help_visible {
        state.spai_notes.scratch.help_visible = false;
        return true;
    }

    // Footer input modes own every key while active.
    match state.spai_notes.scratch.input_mode {
        Some(ScratchInput::FuzzyFilter) => {
            return super::scratch_input::handle_filter_input(key, state)
        }
        Some(ScratchInput::SemanticFilter) => {
            return super::scratch_input::handle_semantic_input(key, state)
        }
        None => {}
    }

    // The dedup popup sits above the buffer; unmatched keys fall through.
    if state.spai_notes.scratch.dedup.visible
        && super::scratch_input::handle_dedup_keys(key, state)
    {
        return true;
    }

    // The @ mention popup sits above the buffer while a token is open.
    if state.spai_notes.scratch.mention.is_some() {
        match key.code {
            KeyCode::Esc => {
                state.spai_notes.scratch.close_mention_popup();
                return true;
            }
            KeyCode::Up => {
                state
                    .spai_notes
                    .scratch
                    .mention_move_selection(&state.spai_notes.projects, -1);
                return true;
            }
            KeyCode::Down => {
                state
                    .spai_notes
                    .scratch
                    .mention_move_selection(&state.spai_notes.projects, 1);
                return true;
            }
            KeyCode::Enter | KeyCode::Tab => {
                if !state
                    .spai_notes
                    .scratch
                    .accept_mention(&state.spai_notes.projects)
                {
                    state.spai_notes.scratch.close_mention_popup();
                }
                return true;
            }
            _ => {} // typing falls through and re-filters the popup
        }
    }

    handle_buffer_keys(key, state)
}

pub fn threshold(state: &SidebarState) -> f64 {
    state.settings.similarity_threshold as f64 / 100.0
}

fn save_scratch(state: &mut SidebarState) {
    state.spai_notes.scratch_save_all();
}

fn run_dedup(state: &mut SidebarState) {
    state.spai_notes.run_scratch_dedup(
        &state.settings.api_key,
        &state.settings.embedding_model,
        threshold(state),
    );
}

// ── The single keymap ─────────────────────────────────────────────

fn handle_buffer_keys(key: &KeyEvent, state: &mut SidebarState) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let scratch = &mut state.spai_notes.scratch;

    match key.code {
        KeyCode::Esc => {
            // Close (twice when unsaved — the draft is kept regardless).
            if scratch.dirty && !scratch.confirm_close {
                scratch.confirm_close = true;
            } else {
                state.spai_notes.close_scratch();
            }
            true
        }
        KeyCode::Char('s') if ctrl => {
            save_scratch(state);
            true
        }
        KeyCode::Char('d') | KeyCode::Char('D') if ctrl => {
            run_dedup(state);
            true
        }
        KeyCode::F(1) => {
            scratch.help_visible = true;
            true
        }
        KeyCode::Char('o') if ctrl => {
            let idx = scratch.cursor_line;
            if let Err(e) = state.spai_notes.open_scratch_record(idx) {
                state.spai_notes.status_message = Some(e);
            }
            true
        }
        KeyCode::Char('x') if ctrl => {
            let idx = scratch.cursor_line;
            if let Err(e) = state.spai_notes.cycle_scratch_record_status(idx) {
                state.spai_notes.status_message = Some(e);
            }
            true
        }
        KeyCode::Char('z') if ctrl => {
            match state.spai_notes.scratch_undo_batch() {
                Ok(n) => {
                    state.spai_notes.status_message = Some(format!("Vráceno {} souborů", n));
                }
                Err(e) => state.spai_notes.status_message = Some(e),
            }
            true
        }
        KeyCode::Char('t') if ctrl => {
            let next = scratch.scope.next();
            state.spai_notes.switch_scratch_scope(next);
            true
        }
        KeyCode::Char('T') if ctrl && key.modifiers.contains(KeyModifiers::SHIFT) => {
            let prev = match scratch.scope {
                ScratchScope::New => ScratchScope::All,
                ScratchScope::Project => ScratchScope::New,
                ScratchScope::All => ScratchScope::Project,
            };
            state.spai_notes.switch_scratch_scope(prev);
            true
        }
        KeyCode::Char('f') if ctrl => {
            scratch.input_mode = Some(ScratchInput::FuzzyFilter);
            scratch.input_buffer.clear();
            true
        }
        KeyCode::Char('r') if ctrl => {
            scratch.input_mode = Some(ScratchInput::SemanticFilter);
            scratch.input_buffer.clear();
            true
        }
        KeyCode::Char('y') if ctrl => {
            scratch.cycle_status_filter();
            true
        }
        KeyCode::Char('l') if ctrl => {
            scratch.filter = None;
            scratch.semantic_allowed = None;
            scratch.status_filter = StatusFilter::All;
            true
        }
        KeyCode::Enter => {
            scratch.split_line();
            scratch.update_mention_popup();
            true
        }
        KeyCode::Backspace => {
            scratch.backspace();
            scratch.update_mention_popup();
            true
        }
        KeyCode::Delete => {
            scratch.forward_delete();
            scratch.update_mention_popup();
            true
        }
        KeyCode::Up => {
            scratch.cursor_up();
            true
        }
        KeyCode::Down => {
            scratch.cursor_down();
            true
        }
        KeyCode::Left => {
            scratch.cursor_char = scratch.cursor_char.saturating_sub(1);
            true
        }
        KeyCode::Right => {
            scratch.cursor_char = (scratch.cursor_char + 1).min(scratch.current_line_len());
            true
        }
        KeyCode::Home => {
            scratch.cursor_char = 0;
            true
        }
        KeyCode::End => {
            scratch.cursor_char = scratch.current_line_len();
            true
        }
        KeyCode::PageUp => {
            for _ in 0..10 {
                scratch.cursor_up();
            }
            true
        }
        KeyCode::PageDown => {
            for _ in 0..10 {
                scratch.cursor_down();
            }
            true
        }
        KeyCode::Tab => {
            let _ = scratch.insert_char(' ');
            let _ = scratch.insert_char(' ');
            scratch.update_mention_popup();
            true
        }
        KeyCode::Char(c) if crate::shared::keys::is_text_input(key) => {
            scratch.insert_char(c);
            scratch.update_mention_popup();
            true
        }
        _ => true, // the Scratchpad owns the keyboard while open
    }
}
