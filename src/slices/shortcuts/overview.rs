//! Standalone plugin-usage overview window (`pi_sidebar usage`).
//!
//! Opened by its own Herdr keybinding as a `placement = "popup"` pane — the same
//! mechanic the Kanban board uses. It renders *only* the overview: no sidebar
//! header, tab bar or shared banner, so it reads as a dedicated modal window and
//! can be dismissed with `Esc`.
use super::modal;
use super::usage::state::UsageOverview;
use crate::shared::TerminalGuard;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::io;
use std::time::Duration;

/// Runs the overview until the user closes it.
///
/// Keys: `←/→` switch panel, `↑/↓` (`k`/`j`) scroll a row, `PgUp/PgDn` (or `Space`) a screen,
/// `Home/End` (`g`/`G`) jump to the bounds, `r` rescans, `Esc`/`q` closes.
pub fn run_overview() -> io::Result<()> {
    let mut state = UsageOverview::new();
    // Cached when the log fingerprint is unchanged, so repeat opens are instant.
    state.ensure_scan(false);

    let mut guard = TerminalGuard::init()?;
    let tick_rate = Duration::from_millis(150);

    loop {
        // Harvest a finished scan before painting so the window can switch from
        // the progress bar to the data in the same frame.
        state.poll_scan();

        guard.terminal_mut().draw(|frame| {
            modal::render_usage_panel(frame, frame.area(), &mut state);
        })?;

        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('r') => state.ensure_scan(true),
                    KeyCode::Left => state.focus_prev(),
                    KeyCode::Right => state.focus_next(),
                    KeyCode::Up | KeyCode::Char('k') => state.scroll_by(-1),
                    KeyCode::Down | KeyCode::Char('j') => state.scroll_by(1),
                    KeyCode::PageUp => state.page(false),
                    KeyCode::PageDown | KeyCode::Char(' ') => state.page(true),
                    KeyCode::Home | KeyCode::Char('g') => state.scroll_home(),
                    KeyCode::End | KeyCode::Char('G') => state.scroll_end(),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    _ => {}
                },
                _ => {}
            }
        }
    }

    Ok(())
}
