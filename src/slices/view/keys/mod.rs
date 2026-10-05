//! Keyboard dispatch for the sidebar event loop.
pub mod creation;
pub mod dialogs;
pub mod edit;

use self::dialogs::{handle_notes_dialogs, handle_settings_dialogs};
use super::external::open_external_editor;
use super::state::{SidebarState, Tab};
use crate::shared::TerminalGuard;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Handles one key press. Returns `true` when the sidebar should quit.
pub fn handle_key(key: KeyEvent, state: &mut SidebarState, guard: &mut TerminalGuard) -> bool {
    if key.kind != KeyEventKind::Press {
        return false;
    }

    if handle_notes_dialogs(&key, state, guard) || handle_settings_dialogs(&key, state) {
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

fn handle_global_keys(key: &KeyEvent, state: &mut SidebarState, guard: &mut TerminalGuard) {
    match key.code {
        KeyCode::Tab => state.next_tab(),
        KeyCode::BackTab => state.prev_tab(),
        KeyCode::Left | KeyCode::Char('h') => {
            if state.active_tab == Tab::Notes {
                state.spai_notes.prev_project();
            } else if state.active_tab == Tab::Settings {
                match state.settings.selected_field {
                    crate::slices::settings::SettingsField::ChatModel => {
                        state.settings.prev_chat_model()
                    }
                    crate::slices::settings::SettingsField::EmbeddingModel => {
                        state.settings.prev_embedding_model()
                    }
                    crate::slices::settings::SettingsField::SimilarityThreshold => {
                        state.settings.decrease_threshold()
                    }
                    _ => state.prev_tab(),
                }
            } else {
                state.prev_tab();
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if state.active_tab == Tab::Notes {
                state.spai_notes.next_project();
            } else if state.active_tab == Tab::Settings {
                match state.settings.selected_field {
                    crate::slices::settings::SettingsField::ChatModel => {
                        state.settings.next_chat_model()
                    }
                    crate::slices::settings::SettingsField::EmbeddingModel => {
                        state.settings.next_embedding_model()
                    }
                    crate::slices::settings::SettingsField::SimilarityThreshold => {
                        state.settings.increase_threshold()
                    }
                    _ => state.next_tab(),
                }
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
        KeyCode::Char('6') => state.set_tab(Tab::Settings),
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
        KeyCode::Delete if state.active_tab == Tab::Notes => {
            state.spai_notes.begin_delete_selected();
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
        KeyCode::Char('v') if state.active_tab == Tab::Settings => {
            crate::slices::settings::vectorize_missing_records(
                &mut state.settings,
                &mut state.spai_notes,
            );
        }
        KeyCode::Char('V') if state.active_tab == Tab::Settings => {
            crate::slices::settings::vectorize_all_records(
                &mut state.settings,
                &mut state.spai_notes,
            );
        }
        KeyCode::Char('f') if state.active_tab == Tab::Settings => {
            crate::slices::settings::classify_missing_facets(
                &mut state.settings,
                &mut state.spai_notes,
            );
        }
        KeyCode::Char('F') if state.active_tab == Tab::Settings => {
            crate::slices::settings::classify_all_facets(
                &mut state.settings,
                &mut state.spai_notes,
            );
        }
        KeyCode::Char('e') if state.active_tab == Tab::Settings => {
            state.settings.start_editing_api_key();
        }
        KeyCode::Char('+') | KeyCode::Char('=') if state.active_tab == Tab::Settings => {
            state.settings.increase_threshold();
        }
        KeyCode::Char('-') | KeyCode::Char('_') if state.active_tab == Tab::Settings => {
            state.settings.decrease_threshold();
        }
        KeyCode::Enter if state.active_tab == Tab::Settings => {
            match state.settings.selected_field {
                crate::slices::settings::SettingsField::ApiKey => {
                    state.settings.start_editing_api_key();
                }
                crate::slices::settings::SettingsField::ChatModel => {
                    state
                        .settings
                        .open_picker(crate::slices::settings::ModelTarget::Chat);
                }
                crate::slices::settings::SettingsField::EmbeddingModel => {
                    state
                        .settings
                        .open_picker(crate::slices::settings::ModelTarget::Embedding);
                }
                crate::slices::settings::SettingsField::VectorizeMissingAction => {
                    crate::slices::settings::vectorize_missing_records(
                        &mut state.settings,
                        &mut state.spai_notes,
                    );
                }
                crate::slices::settings::SettingsField::VectorizeAllAction => {
                    crate::slices::settings::vectorize_all_records(
                        &mut state.settings,
                        &mut state.spai_notes,
                    );
                }
                crate::slices::settings::SettingsField::ClassifyMissingAction => {
                    crate::slices::settings::classify_missing_facets(
                        &mut state.settings,
                        &mut state.spai_notes,
                    );
                }
                crate::slices::settings::SettingsField::ClassifyAllAction => {
                    crate::slices::settings::classify_all_facets(
                        &mut state.settings,
                        &mut state.spai_notes,
                    );
                }
                _ => {}
            }
        }
        KeyCode::Up | KeyCode::Char('k') => match state.active_tab {
            Tab::Notes => state.spai_notes.prev_item(),
            Tab::Shortcuts => state.shortcuts.prev(),
            Tab::Settings => {
                state.settings.selected_field = state.settings.selected_field.prev();
            }
            _ => state.scroll_up(1),
        },
        KeyCode::Down | KeyCode::Char('j') => match state.active_tab {
            Tab::Notes => state.spai_notes.next_item(),
            Tab::Shortcuts => state.shortcuts.next(),
            Tab::Settings => {
                state.settings.selected_field = state.settings.selected_field.next();
            }
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
            } else if state.active_tab == Tab::Settings {
                state.settings.refresh_models();
            } else {
                state.trigger_manual_refresh();
            }
        }
        KeyCode::Char('w') => state.cycle_weather_location(),
        _ => {}
    }
}
