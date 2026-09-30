//! Dev previews for the SPAI Notes face — they read the real project cache.
//!
//!   cargo test preview_project_picker -- --ignored --nocapture
//!
//! Prints the rendered cells of the creation dialog with the `@` picker open,
//! which is the surface that used to trigger a full rescan of every project.
use super::*;
use crate::slices::spai_notes::state::SpaiNotesState;
use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
use std::path::PathBuf;
use std::time::Instant;

fn buffer_text(buffer: &Buffer) -> String {
    let area = buffer.area;
    let mut out = String::new();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn render(state: &SpaiNotesState, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal
        .draw(|frame| render_spai_notes_tab(frame, frame.area(), state))
        .expect("draw");
    buffer_text(terminal.backend().buffer())
}

#[test]
#[ignore = "dev preview, reads the real project cache"]
fn preview_project_picker() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let t = Instant::now();
    let mut state = SpaiNotesState::new(Some(cwd.clone()));
    println!(
        "discovery: {:?} for {} projects",
        t.elapsed(),
        state.projects.len()
    );

    // An idle tick must not touch a single note file.
    let t = Instant::now();
    state.refresh(Some(&cwd), false);
    println!("idle refresh (gated): {:?}", t.elapsed());

    state.open_creation_dialog();
    for c in ". @a".chars() {
        state.on_dialog_char_typed(c);
    }

    println!("{}", render(&state, 90, 22));
}
