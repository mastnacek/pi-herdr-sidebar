//! Dev previews for the SPAI Notes face — they read the real project cache.
//!
//!   cargo test preview_project_picker -- --ignored --nocapture
//!
//! Prints the rendered cells of the creation dialog with the `@` picker open,
//! which is the surface that used to trigger a full rescan of every project.
use super::*;
use crate::slices::spai_notes::discovery::SpaiProjectSummary;
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

    println!("== split pane (90x22) ==");
    println!("{}", render(&state, 90, 22));
    println!("== notes modal (130x40) ==");
    println!("{}", render(&state, 130, 40));
}

/// Deterministic project list — no disk, no project cache involved.
fn state_with_projects(count: usize) -> SpaiNotesState {
    let mut state = SpaiNotesState::new(None);
    state.projects = (0..count)
        .map(|i| {
            SpaiProjectSummary::new(
                format!("proj-{i:02}"),
                PathBuf::from(format!("D:/tmp/proj-{i:02}")),
                PathBuf::from("D:/tmp/no-notes"),
            )
        })
        .collect();
    state
}

#[test]
fn the_project_picker_scrolls_to_the_last_match() {
    let mut state = state_with_projects(12);
    state.open_creation_dialog();
    state.on_dialog_char_typed('@');
    assert_eq!(state.creation_dialog.suggestions.len(), 12, "no hard cap");

    for _ in 0..11 {
        state.next_suggestion();
    }
    assert_eq!(state.creation_dialog.autocomplete_selected, 11);

    let frame = render(&state, 90, 22);
    assert!(
        frame.contains("proj-11"),
        "last project is reachable:\n{frame}"
    );
    assert!(frame.contains("[12/12]"), "position shown:\n{frame}");
    assert!(
        !frame.contains("@proj-00"),
        "the tail scrolled out instead of overflowing the box:\n{frame}"
    );
}

#[test]
fn a_short_list_needs_no_scrolling() {
    let mut state = state_with_projects(3);
    state.open_creation_dialog();
    state.on_dialog_char_typed('@');
    let frame = render(&state, 90, 22);
    assert!(frame.contains("proj-00"), "first match visible:\n{frame}");
    assert!(frame.contains("proj-02"), "last match visible:\n{frame}");
}

#[test]
fn item_list_renders_short_date_instead_of_spai_id() {
    let mut state = SpaiNotesState::new(None);
    let item = crate::slices::spai_notes::note::SpaiNoteItem {
        id: "SPAI-042".to_string(),
        title: "Test Note Title".to_string(),
        kind: crate::slices::spai_notes::note::SpaiType::Todo,
        status: crate::slices::spai_notes::note::SpaiStatus::Todo,
        symbol: ".".to_string(),
        timestamp: "2026-09-24 10:57:47".to_string(),
        tags: vec![],
        facets: crate::slices::spai_notes::note::SpaiFacets {
            project: None,
            project_path: None,
            priority: None,
            deadline: None,
            ..Default::default()
        },
        body: ". Test Note Title\n".to_string(),
        file_path: PathBuf::from("2026-09-24-SPAI-042-test.md"),
        file_name: "2026-09-24-SPAI-042-test.md".to_string(),
    };
    state.projects = vec![SpaiProjectSummary::with_items(
        "proj-01".to_string(),
        PathBuf::from("D:/tmp/proj-01"),
        PathBuf::from("D:/tmp/proj-01/docs/spai"),
        vec![item],
    )];

    let frame = render(&state, 90, 22);
    // Left pane must display the shortened timestamp in dd.mm.yy format: 24.09.26
    assert!(
        frame.contains("24.09.26"),
        "frame should contain formatted short date '24.09.26':\n{frame}"
    );
    // Left pane list row must NOT contain the old "SPAI-042" column format (though the viewer header on the right pane shows full info)
    assert!(
        frame.contains("▶ [.] 24.09.26 Test Note Title"),
        "list row should have marker, glyph, dd.mm.yy date, and title:\n{frame}"
    );
}

#[test]
fn viewer_preview_hides_the_file_name() {
    // The item list already identifies the note; the preview must not waste a
    // row repeating the file name.
    let mut state = SpaiNotesState::new(None);
    let item = crate::slices::spai_notes::note::SpaiNoteItem {
        id: "SPAI-042".to_string(),
        title: "Test Note Title".to_string(),
        kind: crate::slices::spai_notes::note::SpaiType::Todo,
        status: crate::slices::spai_notes::note::SpaiStatus::Todo,
        symbol: ".".to_string(),
        timestamp: "2026-09-24 10:57:47".to_string(),
        tags: vec![],
        facets: crate::slices::spai_notes::note::SpaiFacets::default(),
        body: ". Test Note Title\n".to_string(),
        file_path: PathBuf::from("2026-09-24-SPAI-042-test.md"),
        file_name: "2026-09-24-SPAI-042-test.md".to_string(),
    };
    state.projects = vec![SpaiProjectSummary::with_items(
        "proj-01".to_string(),
        PathBuf::from("D:/tmp/proj-01"),
        PathBuf::from("D:/tmp/proj-01/docs/spai"),
        vec![item],
    )];

    let frame = render(&state, 90, 22);
    assert!(
        frame.contains("SPAI-042"),
        "viewer header should still show the id:\n{frame}"
    );
    assert!(
        !frame.contains("2026-09-24-SPAI-042-test.md"),
        "preview must not render the file name:\n{frame}"
    );
}
