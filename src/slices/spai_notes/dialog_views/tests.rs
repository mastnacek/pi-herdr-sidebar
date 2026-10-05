use super::creation::{picker_viewport, render_creation_dialog};
use crate::slices::spai_notes::state::SpaiNotesState;
use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

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

#[test]
fn the_viewport_grows_with_the_box_and_follows_the_selection() {
    // 8-row box → 6 rows of projects.
    assert_eq!(picker_viewport(12, 0, 8), (6, 0));
    assert_eq!(picker_viewport(12, 5, 8), (6, 0));
    assert_eq!(picker_viewport(12, 6, 8), (6, 1));
    assert_eq!(picker_viewport(12, 11, 8), (6, 6));
    // A short list fits whole, no scrolling.
    assert_eq!(picker_viewport(3, 2, 8), (3, 0));
    // A taller box shows more rows.
    assert_eq!(picker_viewport(12, 11, 14), (12, 0));
}

#[test]
fn creation_dialog_renders_type_list_and_hint_window_together() {
    let mut state = SpaiNotesState::new(None);
    state.open_creation_dialog();

    let mut terminal = Terminal::new(TestBackend::new(88, 24)).expect("terminal");
    terminal
        .draw(|frame| render_creation_dialog(frame, frame.area(), &state))
        .expect("draw");
    let frame = buffer_text(terminal.backend().buffer());

    assert!(frame.contains("Typ záznamu"), "type list block present");
    assert!(frame.contains("Kontext: Úkol (pending)"), "hint window header present");
    assert!(frame.contains("Příklady zápisu"), "examples subheader present");
    assert!(frame.contains("Zadání"), "input box present");
}

