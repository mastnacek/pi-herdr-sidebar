pub mod mcp;
pub mod shared_banner;
pub mod skills;
pub mod spai_ui;
pub mod state;
pub mod state_model;
pub mod state_refresh;
pub mod state_resolver;
pub mod status;
pub mod ui;
pub mod weather_ui;
pub mod zen;

use crate::shared::{PidLock, TerminalGuard};
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use state::{SidebarState, Tab};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Pane,
    Popup,
    Renderer,
}

pub fn run_view(mode: ViewMode, snapshot_override: Option<PathBuf>) -> io::Result<()> {
    let mut state = SidebarState::new(snapshot_override);

    // If we have a snapshot file, acquire single-instance lock to avoid multiple renderers racing
    let _lock = if let Some(path) = &state.snapshot_path {
        PidLock::acquire(path)
    } else {
        None
    };

    let mut guard = TerminalGuard::init()?;
    let terminal = guard.terminal_mut();

    let tick_rate = Duration::from_millis(150);

    loop {
        terminal.draw(|f| {
            ui::render(f, &state);
        })?;

        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) => {
                    // Windows terminals emit Press AND Release events; react to
                    // presses only, otherwise 'w' cycles two locations per keystroke.
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    // If creation dialog is active, intercept dialog keystrokes
                    if state.active_tab == Tab::Notes && state.spai_notes.creation_dialog.active {
                        let ac_active = state.spai_notes.creation_dialog.autocomplete_active;
                        match key.code {
                            KeyCode::Esc => {
                                if ac_active {
                                    state.spai_notes.creation_dialog.autocomplete_active = false;
                                } else {
                                    state.spai_notes.close_creation_dialog();
                                }
                            }
                            KeyCode::Up if ac_active => {
                                state.spai_notes.prev_suggestion();
                            }
                            KeyCode::Down if ac_active => {
                                state.spai_notes.next_suggestion();
                            }
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
                            KeyCode::Backspace => {
                                state.spai_notes.on_dialog_backspace();
                            }
                            KeyCode::Char(c) => {
                                state.spai_notes.on_dialog_char_typed(c);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Quit on q or Esc, or Ctrl+c
                    if key.code == KeyCode::Char('q')
                        || key.code == KeyCode::Esc
                        || (key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.code == KeyCode::Char('c'))
                    {
                        break;
                    }

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
                        KeyCode::Char('x') if state.active_tab == Tab::Notes => {
                            let _ = state.spai_notes.cycle_selected_status();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if state.active_tab == Tab::Notes {
                                state.spai_notes.prev_item();
                            } else {
                                state.scroll_up(1);
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if state.active_tab == Tab::Notes {
                                state.spai_notes.next_item();
                            } else {
                                state.scroll_down(1);
                            }
                        }
                        KeyCode::PageUp => {
                            if state.active_tab == Tab::Notes {
                                state.spai_notes.scroll_viewer_up(6);
                            } else {
                                state.scroll_up(10);
                            }
                        }
                        KeyCode::PageDown => {
                            if state.active_tab == Tab::Notes {
                                state.spai_notes.scroll_viewer_down(6);
                            } else {
                                state.scroll_down(10);
                            }
                        }
                        KeyCode::Char('d') if state.active_tab == Tab::Notes => {
                            state.spai_notes.scroll_viewer_down(4);
                        }
                        KeyCode::Char('u') if state.active_tab == Tab::Notes => {
                            state.spai_notes.scroll_viewer_up(4);
                        }
                        KeyCode::Home => state.scroll = 0,
                        KeyCode::Char('r') => state.trigger_manual_refresh(),
                        KeyCode::Char('w') => {
                            // Rotate to next weather location and refetch
                            state.cycle_weather_location();
                        }
                        KeyCode::Char('c') => {
                            // Copy full forecast (all locations × all days) to clipboard
                            let report =
                                crate::slices::telemetry::weather_live::collect_all_locations_report();
                            let ok =
                                crate::slices::telemetry::weather_live::copy_to_clipboard(&report);
                            state.refresh_status = if ok {
                                "📋 Předpověď zkopírována do schránky".to_string()
                            } else {
                                "Schránka není dostupná".to_string()
                            };
                            state.refresh_timer = 8;
                            state.refresh_progress = 0.0;
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        state.handle_mouse_click(mouse.column, mouse.row);
                    }
                    MouseEventKind::ScrollUp => {
                        if state.active_tab == Tab::Notes {
                            // If mouse is on right pane, scroll viewer, else previous item
                            if mouse.column >= 30 {
                                state.spai_notes.scroll_viewer_up(3);
                            } else {
                                state.spai_notes.prev_item();
                            }
                        } else {
                            state.scroll_up(2);
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if state.active_tab == Tab::Notes {
                            if mouse.column >= 30 {
                                state.spai_notes.scroll_viewer_down(3);
                            } else {
                                state.spai_notes.next_item();
                            }
                        } else {
                            state.scroll_down(2);
                        }
                    }
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        } else {
            // Tick: update animation & check for snapshot changes
            state.tick_animation();
            state.refresh(false);
        }

        // If in renderer mode and snapshot marked live=false, we keep the last frame on screen unless quit
        if mode == ViewMode::Renderer {
            if let Some(snap) = &state.snapshot {
                if !snap.live {
                    // session ended
                }
            }
        }
    }

    Ok(())
}
