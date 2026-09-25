//! Unit tests for the SPAI notes edit dialog (cursor-aware body/title editing).
use super::*;
use std::fs;

fn temp_project() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("spai_edit_test_{}_{}", std::process::id(), nanos));
    fs::create_dir_all(dir.join("docs").join("spai")).unwrap();
    dir
}

#[test]
fn edit_dialog_persists_title_and_body() {
    let project = temp_project();
    let note_path = project
        .join("docs")
        .join("spai")
        .join("2026-01-01-SPAI-001-hello.md");
    fs::write(
            &note_path,
            "---\ntype: Todo\ntitle: \"Hello\"\ntimestamp: 1\nstatus: todo\nsource: pi-spai\nspai_symbol: '.'\n---\n\n# SPAI-001: Hello\n. Hello\n",
        )
        .unwrap();

    let mut state = SpaiNotesState::new(Some(project.clone()));
    let idx = state
        .projects
        .iter()
        .position(|p| p.path == project)
        .expect("temp project discovered");
    state.selected_project_idx = idx;
    assert!(state.selected_item().is_some());

    state.open_edit_dialog();
    assert!(state.edit_dialog.active);
    state.edit_dialog.title_input = "Renamed".to_string();
    state.edit_dialog.body_input = ". Renamed\nSecond line\n".to_string();
    state.submit_edit_dialog().unwrap();

    assert!(!state.edit_dialog.active, "dialog closes after save");
    let written = fs::read_to_string(&note_path).unwrap();
    assert!(
        written.contains("title: \"Renamed\""),
        "frontmatter title: {written}"
    );
    assert!(written.contains("# SPAI-001: Renamed"), "header: {written}");
    assert!(written.contains(". Renamed"), "body prefix line: {written}");
    assert!(
        written.contains("Second line"),
        "edited body line: {written}"
    );

    fs::remove_dir_all(&project).ok();
}

#[test]
fn edit_dialog_rejects_empty_title() {
    let mut state = SpaiNotesState::new(None);
    state.edit_dialog.title_input = "   ".to_string();
    let err = state.submit_edit_dialog().unwrap_err();
    assert!(err.contains("Název"), "unexpected error: {err}");
}

/// Body-focused dialog with a known two-line buffer.
fn body_editor(body: &str) -> SpaiNotesState {
    let mut state = SpaiNotesState::new(None);
    state.edit_dialog.active = true;
    state.edit_dialog.body_input = body.to_string();
    state.edit_dialog.field = EditField::Body;
    state.edit_dialog.cursor = 0;
    state.edit_dialog.body_follow = true;
    state
}

#[test]
fn typing_inserts_at_cursor_not_at_end() {
    let mut state = body_editor("line one\nline two");
    // cursor starts at 0 → prepend, not append
    state.on_edit_char_typed('X');
    assert_eq!(state.edit_dialog.body_input, "Xline one\nline two");
    assert_eq!(state.edit_dialog.cursor, 1);

    // move to the very end and append
    state.on_edit_cursor_line_end();
    state.on_edit_cursor_vertical(true);
    state.on_edit_cursor_line_end();
    state.on_edit_char_typed('!');
    assert_eq!(state.edit_dialog.body_input, "Xline one\nline two!");
}

#[test]
fn backspace_and_delete_work_around_the_cursor() {
    let mut state = body_editor("abcdef");
    state.edit_dialog.cursor = 3; // between c and d
    state.on_edit_backspace();
    assert_eq!(state.edit_dialog.body_input, "abdef");
    assert_eq!(state.edit_dialog.cursor, 2);
    state.on_edit_delete();
    assert_eq!(state.edit_dialog.body_input, "abef");
    assert_eq!(state.edit_dialog.cursor, 2);
}

#[test]
fn line_navigation_keeps_the_column_and_clamps() {
    let mut state = body_editor("short\nmuch longer line");
    state.edit_dialog.cursor = 5; // end of "short"
    state.on_edit_cursor_vertical(true);
    assert_eq!(state.edit_dialog.cursor, 11, "col 5 on the longer line");
    state.on_edit_cursor_vertical(false);
    assert_eq!(state.edit_dialog.cursor, 5, "back up on the short line");

    // clamps to the shorter line's end instead of overshooting
    state.edit_dialog.cursor = 16;
    state.on_edit_cursor_vertical(false);
    assert_eq!(state.edit_dialog.cursor, 5);
}

#[test]
fn home_end_are_line_scoped_and_enter_inserts_newline() {
    let mut state = body_editor("alpha\nbeta");
    state.edit_dialog.cursor = 9; // end of "beta"
    state.on_edit_cursor_line_start();
    assert_eq!(state.edit_dialog.cursor, 6);
    state.on_edit_enter();
    assert_eq!(state.edit_dialog.body_input, "alpha\n\nbeta");
    assert_eq!(
        state.edit_dialog.cursor, 7,
        "cursor advances past the new line"
    )
}

#[test]
fn manual_scroll_pauses_follow_and_editing_restores_it() {
    let mut state = body_editor("a\nb");
    assert!(state.edit_dialog.body_follow);
    state.scroll_edit_body(true, 5);
    assert!(!state.edit_dialog.body_follow, "PgDn pauses auto-follow");
    assert_eq!(state.edit_dialog.body_scroll, 5);
    state.on_edit_char_typed('z');
    assert!(state.edit_dialog.body_follow, "typing resumes auto-follow");
}

#[test]
fn toggle_field_parks_cursor_at_end_of_new_field() {
    let mut state = body_editor("body text");
    state.edit_dialog.title_input = "My title".to_string();
    state.toggle_edit_field();
    assert_eq!(state.edit_dialog.field, EditField::Title);
    assert_eq!(state.edit_dialog.cursor, 8);
    state.toggle_edit_field();
    assert_eq!(state.edit_dialog.field, EditField::Body);
    assert_eq!(state.edit_dialog.cursor, 9);
}

#[test]
fn multibyte_titles_are_edited_by_char_index() {
    let mut state = body_editor("");
    state.edit_dialog.field = EditField::Title;
    state.edit_dialog.title_input = "příliš".to_string();
    state.edit_dialog.cursor = 0;
    state.on_edit_char_typed('X');
    assert_eq!(state.edit_dialog.title_input, "Xpříliš");

    // cursor 3 sits after the two-byte 'ř', so backspace removes it whole
    state.on_edit_cursor_right();
    state.on_edit_cursor_right();
    state.on_edit_backspace();
    assert_eq!(state.edit_dialog.title_input, "Xpíliš");
    assert_eq!(state.edit_dialog.cursor, 2);
}
