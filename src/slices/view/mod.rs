//! Interactive sidebar TUI: event loop, terminal lifecycle and mouse handling.
//!
//! Keyboard dispatch lives in [`keys`] and the external-editor hand-off in
//! [`external`], so this module stays a thin loop.
pub mod external;
pub mod keys;
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
use crossterm::event::{self, Event, MouseButton, MouseEventKind};
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

/// Startup options derived from the CLI so a Herdr entrypoint can open a
/// specific face directly (e.g. `popup --tab notes`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Startup {
    pub tab: Option<Tab>,
}

pub fn run_view(
    mode: ViewMode,
    snapshot_override: Option<PathBuf>,
    startup: Startup,
) -> io::Result<()> {
    let mut state = SidebarState::new(snapshot_override);
    if let Some(tab) = startup.tab {
        state.set_tab(tab);
    }

    // If we have a snapshot file, acquire single-instance lock to avoid multiple renderers racing
    let _lock = if let Some(path) = &state.snapshot_path {
        PidLock::acquire(path)
    } else {
        None
    };

    let mut guard = TerminalGuard::init()?;

    let tick_rate = Duration::from_millis(150);

    loop {
        guard.terminal_mut().draw(|f| {
            ui::render(f, &state);
        })?;

        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) => {
                    if keys::handle_key(key, &mut state, &mut guard) {
                        break;
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
                        } else if state.active_tab == Tab::Shortcuts {
                            state.shortcuts.prev();
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
                        } else if state.active_tab == Tab::Shortcuts {
                            state.shortcuts.next();
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
            // Tick: update animation and check for snapshot changes
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
