pub mod state;
pub mod ui;

use crate::shared::{PidLock, TerminalGuard};
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
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
                        KeyCode::Left | KeyCode::Char('h') => state.prev_tab(),
                        KeyCode::Right | KeyCode::Char('l') => state.next_tab(),
                        KeyCode::Char('1') => state.set_tab(Tab::Status),
                        KeyCode::Char('2') => state.set_tab(Tab::Skills),
                        KeyCode::Char('3') => state.set_tab(Tab::Mcp),
                        KeyCode::Up | KeyCode::Char('k') => state.scroll_up(1),
                        KeyCode::Down | KeyCode::Char('j') => state.scroll_down(1),
                        KeyCode::PageUp => state.scroll_up(10),
                        KeyCode::PageDown => state.scroll_down(10),
                        KeyCode::Home => state.scroll = 0,
                        KeyCode::Char('r') => state.trigger_manual_refresh(),
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        state.handle_mouse_click(mouse.column, mouse.row);
                    }
                    MouseEventKind::ScrollUp => {
                        state.scroll_up(2);
                    }
                    MouseEventKind::ScrollDown => {
                        state.scroll_down(2);
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
