//! Footer input modes of the Scratchpad (plan §4): `/` fuzzy and `~`
//! semantic filters, plus the dedup popup keys. Split from scratch.rs for
//! the line cap.
use super::super::state::SidebarState;
use crate::slices::spai_notes::scratch::filter::StatusFilter;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn threshold(state: &SidebarState) -> f64 {
    state.settings.similarity_threshold as f64 / 100.0
}

/// `/` fuzzy filter: typing accumulates, Enter applies, Esc cancels.
pub fn handle_filter_input(key: &KeyEvent, state: &mut SidebarState) -> bool {
    let scratch = &mut state.spai_notes.scratch;
    match key.code {
        KeyCode::Enter => {
            let text = scratch.input_buffer.trim().to_string();
            scratch.input_mode = None;
            scratch.input_buffer.clear();
            scratch.apply_filter_text(if text.is_empty() { None } else { Some(text) });
            scratch.last_summary = Some(format!("Filtr: {} záznamů", scratch.visible_count()));
            true
        }
        KeyCode::Esc => {
            scratch.input_mode = None;
            scratch.input_buffer.clear();
            true
        }
        KeyCode::Backspace => {
            scratch.input_buffer.pop();
            true
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            scratch.input_buffer.push(c);
            true
        }
        _ => true,
    }
}

/// `~` semantic filter: on Enter — one query embedding + cosine over the
/// stored vectors of loaded records (plan §4). Never runs by itself.
pub fn handle_semantic_input(key: &KeyEvent, state: &mut SidebarState) -> bool {
    let th = threshold(state);
    match key.code {
        KeyCode::Enter => {
            let text = state.spai_notes.scratch.input_buffer.trim().to_string();
            state.spai_notes.scratch.input_mode = None;
            state.spai_notes.scratch.input_buffer.clear();
            if text.is_empty() {
                return true;
            }
            match state.spai_notes.scratch_semantic_filter(
                &text,
                &state.settings.api_key,
                &state.settings.embedding_model,
                th,
            ) {
                Ok(hits) => {
                    state.spai_notes.scratch.last_summary =
                        Some(format!("Sémantický filtr: {} shod", hits));
                }
                Err(e) => {
                    state.spai_notes.scratch.last_summary = Some(format!("⚠ {}", e));
                }
            }
            true
        }
        KeyCode::Esc => {
            state.spai_notes.scratch.input_mode = None;
            state.spai_notes.scratch.input_buffer.clear();
            true
        }
        KeyCode::Backspace => {
            state.spai_notes.scratch.input_buffer.pop();
            true
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.spai_notes.scratch.input_buffer.push(c);
            true
        }
        _ => true,
    }
}

/// Dedup popup keys (plan §3): ↑/↓ select, Ctrl+O open, Ctrl+A append,
/// Ctrl+U cycle status, Esc close. Returns `false` for keys it does not
/// handle so they can fall through to the mode handlers.
pub fn handle_dedup_keys(key: &KeyEvent, state: &mut SidebarState) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => {
            state.spai_notes.scratch.close_dedup();
            true
        }
        KeyCode::Up => {
            state.spai_notes.scratch_dedup_move(false);
            true
        }
        KeyCode::Down => {
            state.spai_notes.scratch_dedup_move(true);
            true
        }
        KeyCode::Char('o') | KeyCode::Char('O') if ctrl => {
            if let Err(e) = state.spai_notes.scratch_dedup_open() {
                state.spai_notes.status_message = Some(e);
            }
            true
        }
        KeyCode::Char('a') | KeyCode::Char('A') if ctrl => {
            if let Err(e) = state.spai_notes.scratch_dedup_append() {
                state.spai_notes.status_message = Some(e);
            }
            true
        }
        KeyCode::Char('u') | KeyCode::Char('U') if ctrl => {
            if let Err(e) = state.spai_notes.scratch_dedup_cycle_status() {
                state.spai_notes.status_message = Some(e);
            }
            true
        }
        _ => false,
    }
}