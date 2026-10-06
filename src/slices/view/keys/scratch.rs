//! Scratchpad key dispatch (docs/scratchpad_mode_plan.md §1–§5, §7).
//!
//! Opened by the global `Ctrl+N`; consumes every key while open. Mode
//! branches: Edit (typing), Read (navigation/filters), the two footer input
//! modes (`/` fuzzy, `~` semantic — in [`super::scratch_input`]) and the
//! dedup popup.
use super::super::state::{SidebarState, Tab};
use crate::slices::spai_notes::scratch::state::{ScratchInput, ScratchMode, ScratchScope};
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
    let _ = Tab::Notes; // kept imported for the dialog handlers upstream

    // The `?` help overlay owns the keyboard while visible: any key closes.
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

    match state.spai_notes.scratch.mode {
        ScratchMode::Edit => handle_edit_key(key, state),
        ScratchMode::Read => handle_read_key(key, state),
    }
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

// ── Edit mode ─────────────────────────────────────────────────────

fn handle_edit_key(key: &KeyEvent, state: &mut SidebarState) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let scratch = &mut state.spai_notes.scratch;

    // `@` autocomplete sits above the buffer while a mention token is open.
    if scratch.mention.is_some() {
        match key.code {
            KeyCode::Esc => {
                scratch.close_mention_popup();
                return true;
            }
            KeyCode::Up => {
                scratch.mention_move_selection(&state.spai_notes.projects, -1);
                return true;
            }
            KeyCode::Down => {
                scratch.mention_move_selection(&state.spai_notes.projects, 1);
                return true;
            }
            KeyCode::Enter | KeyCode::Tab => {
                if !scratch.accept_mention(&state.spai_notes.projects) {
                    scratch.close_mention_popup();
                }
                return true;
            }
            _ => {} // typing falls through and re-filters the popup below
        }
    }

    match key.code {
        KeyCode::Esc => {
            // Edit → Read (the draft is autosaved on close/scope switches).
            scratch.mode = ScratchMode::Read;
            scratch.confirm_close = false;
            scratch.close_mention_popup();
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
        KeyCode::Char('?') => {
            scratch.help_visible = true;
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

// ── Read mode ─────────────────────────────────────────────────────

fn handle_read_key(key: &KeyEvent, state: &mut SidebarState) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let s = &mut state.spai_notes.scratch;

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') if !ctrl => {
            if s.dirty && !s.confirm_close {
                s.confirm_close = true;
            } else {
                state.spai_notes.close_scratch();
            }
            true
        }
        KeyCode::Char('i') if !ctrl => {
            s.mode = ScratchMode::Edit;
            s.confirm_close = false;
            true
        }
        KeyCode::Up | KeyCode::Char('k') if !ctrl => {
            s.move_cursor_by_record(-1);
            true
        }
        KeyCode::Down | KeyCode::Char('j') if !ctrl => {
            s.move_cursor_by_record(1);
            true
        }
        KeyCode::Char('g') if !ctrl => {
            s.jump_to_edge(true);
            true
        }
        KeyCode::Char('G') if !ctrl => {
            s.jump_to_edge(false);
            true
        }
        KeyCode::Tab => {
            let next = s.scope.next();
            state.spai_notes.switch_scratch_scope(next);
            true
        }
        KeyCode::BackTab => {
            let prev = match s.scope {
                ScratchScope::New => ScratchScope::All,
                ScratchScope::Project => ScratchScope::New,
                ScratchScope::All => ScratchScope::Project,
            };
            state.spai_notes.switch_scratch_scope(prev);
            true
        }
        KeyCode::Enter | KeyCode::Char('o') if !ctrl => {
            let idx = s.cursor_line;
            if let Err(e) = state.spai_notes.open_scratch_record(idx) {
                state.spai_notes.status_message = Some(e);
            }
            true
        }
        KeyCode::Char('x') | KeyCode::Char('s') if !ctrl => {
            let idx = s.cursor_line;
            if let Err(e) = state.spai_notes.cycle_scratch_record_status(idx) {
                state.spai_notes.status_message = Some(e);
            }
            true
        }
        KeyCode::Char('u') if !ctrl => {
            match state.spai_notes.scratch_undo_batch() {
                Ok(n) => {
                    state.spai_notes.status_message = Some(format!("Vráceno {} souborů", n));
                }
                Err(e) => state.spai_notes.status_message = Some(e),
            }
            true
        }
        KeyCode::Char('?') => {
            s.help_visible = true;
            true
        }
        KeyCode::Char('/') if !ctrl => {
            s.input_mode = Some(ScratchInput::FuzzyFilter);
            s.input_buffer.clear();
            true
        }
        KeyCode::Char('~') if !ctrl => {
            s.input_mode = Some(ScratchInput::SemanticFilter);
            s.input_buffer.clear();
            true
        }
        KeyCode::Char('f') if !ctrl => {
            s.cycle_status_filter();
            true
        }
        KeyCode::Char('F') if !ctrl => {
            s.filter = None;
            s.semantic_allowed = None;
            s.status_filter = crate::slices::spai_notes::scratch::filter::StatusFilter::All;
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
        _ => true, // the Scratchpad owns the keyboard while open
    }
}