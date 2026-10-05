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

/// Dev preview: `cargo test creation_dialog_full_tab_preview -- --ignored --nocapture`.
/// Renders the dialog on a tall pane (taller than the old 36-row clamp) so
/// bleed-through of the tab UI outside the modal is visible immediately.
#[test]
#[ignore]
fn creation_dialog_full_tab_preview() {
    let mut state = SpaiNotesState::new(None);
    state.open_creation_dialog();

    let mut terminal = Terminal::new(TestBackend::new(84, 44)).expect("terminal");
    terminal
        .draw(|frame| render_creation_dialog(frame, frame.area(), &state))
        .expect("draw");
    let frame = buffer_text(terminal.backend().buffer());
    println!("{frame}");

    // The dialog border must touch the outermost rows: nothing above or below.
    let first = frame.lines().next().unwrap_or("");
    let last = frame.lines().last().unwrap_or("");
    assert!(first.contains('╭'), "dialog starts at row 0");
    assert!(last.contains('╰'), "dialog ends at the last row");
}

#[test]
fn creation_dialog_shows_overview_when_empty_and_footer_when_typing() {
    // Empty input: the dim overview (complete type list + syntax) fills the
    // editor surface; no side panels anywhere (plan §1).
    let mut state = SpaiNotesState::new(None);
    state.open_creation_dialog();

    let mut terminal = Terminal::new(TestBackend::new(88, 24)).expect("terminal");
    terminal
        .draw(|frame| render_creation_dialog(frame, frame.area(), &state))
        .expect("draw");
    let empty = buffer_text(terminal.backend().buffer());

    assert!(empty.contains("Typy"), "type overview present: {empty}");
    assert!(empty.contains("@projekt"), "syntax line present: {empty}");
    assert!(empty.contains("✍"), "editor header present: {empty}");
    assert!(!empty.contains("Typ záznamu"), "side type list must be gone: {empty}");
    assert!(!empty.contains("Kontext:"), "context panel must be gone: {empty}");

    // Typed input: the footer describes only the selected type (↑↓ selection,
    // here `?` = idea at index 5 to match the typed prefix).
    state.creation_dialog.type_selection = 5;
    state.creation_dialog.title_input = "? novy napad".to_string();
    terminal
        .draw(|frame| render_creation_dialog(frame, frame.area(), &state))
        .expect("draw");
    let typed = buffer_text(terminal.backend().buffer());

    assert!(
        typed.contains("Nápad (idea)"),
        "footer follows the typed prefix: {typed}"
    );
    assert!(!typed.contains("Typy"), "overview hidden while typing: {typed}");
}

#[test]
fn dedup_panel_renders_over_the_dialog_and_disappears() {
    let mut state = SpaiNotesState::new(None);
    state.open_creation_dialog();
    state.creation_dialog.title_input = ". nákupní koš".to_string();

    let mut terminal = Terminal::new(TestBackend::new(88, 24)).expect("terminal");
    let mut draw = |st: &SpaiNotesState| {
        let mut t = Terminal::new(TestBackend::new(88, 24)).expect("terminal");
        t.draw(|frame| render_creation_dialog(frame, frame.area(), st)).expect("draw");
        buffer_text(t.backend().buffer())
    };

    // Hidden until Ctrl+D is pressed.
    let before = draw(&state);
    assert!(!before.contains("duplicity") || !before.contains("Podobné"), "panel hidden: {before}");

    // Visible after the (local fallback) run — no project, so "žádné duplicity".
    state.run_dedup("", "model", 0.5, ". nákupní koš".to_string());
    let visible = draw(&state);
    assert!(visible.contains("Kontrola duplicit") || visible.contains("Žádné duplicity") || visible.contains("Podobné"), "panel shown: {visible}");

    state.close_dedup_panel();
    let closed = draw(&state);
    assert!(!closed.contains("Kontrola duplicit"), "panel hidden after Esc: {closed}");
}

