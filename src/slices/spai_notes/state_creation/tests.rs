use crate::slices::spai_notes::note::SpaiType;
use crate::slices::spai_notes::state::SpaiNotesState;
use crate::slices::spai_notes::type_options::SPAI_TYPE_OPTIONS;

#[test]
fn type_selection_cycles_forward_and_backward() {
    let mut state = SpaiNotesState::new(None);
    state.open_creation_dialog();
    assert_eq!(state.creation_dialog.type_selection, 0);
    assert_eq!(state.creation_dialog.selected_kind, SpaiType::Todo);

    state.next_type();
    assert_eq!(state.creation_dialog.type_selection, 1);
    assert_eq!(state.creation_dialog.selected_kind, SpaiType::Todo);

    // Cycle all the way through
    for _ in 0..SPAI_TYPE_OPTIONS.len() - 1 {
        state.next_type();
    }
    assert_eq!(state.creation_dialog.type_selection, 0);

    // Prev type wraps to last
    state.prev_type();
    assert_eq!(state.creation_dialog.type_selection, SPAI_TYPE_OPTIONS.len() - 1);
}

#[test]
fn apply_selected_type_inserts_prefix_only_into_empty_input() {
    let mut state = SpaiNotesState::new(None);
    state.open_creation_dialog();

    // Select idea (?) which is index 5, then Tab inserts the prefix.
    state.creation_dialog.type_selection = 5;
    state.apply_selected_type();
    assert_eq!(state.creation_dialog.title_input, "? ");

    // Plan §1: Tab/↑↓ only apply while the input is empty — once text exists,
    // the type is detected from the typed prefix and apply is a no-op.
    state.creation_dialog.title_input.push_str("novy napad");
    state.creation_dialog.type_selection = 6;
    state.apply_selected_type();
    assert_eq!(
        state.creation_dialog.title_input, "? novy napad",
        "non-empty input must not be rewritten by apply_selected_type"
    );
}

#[test]
fn typing_prefix_syncs_type_selection() {
    let mut state = SpaiNotesState::new(None);
    state.open_creation_dialog();

    state.on_dialog_char_typed('?');
    state.on_dialog_char_typed(' ');
    assert_eq!(state.creation_dialog.type_selection, 5); // Idea
    assert_eq!(state.creation_dialog.selected_kind, SpaiType::Idea);

    state.on_dialog_backspace();
    state.on_dialog_backspace();
    state.on_dialog_char_typed('!');
    state.on_dialog_char_typed('-');
    state.on_dialog_char_typed(' ');
    assert_eq!(state.creation_dialog.type_selection, 6); // Note (with priority !)
    assert_eq!(state.creation_dialog.selected_kind, SpaiType::Note);
}

use std::fs;

fn temp_project() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("spai_create_{}_{}", std::process::id(), nanos));
    fs::create_dir_all(dir.join("docs").join("spai")).unwrap();
    dir
}

fn submit_title(word: &str) -> String {
    let project = temp_project();
    let mut state = SpaiNotesState::new(Some(project.clone()));
    let idx = state
        .projects
        .iter()
        .position(|p| p.path == project)
        .expect("temp project discovered");
    state.selected_project_idx = idx;
    state.open_creation_dialog();
    // Title is set directly (no keystroke sync) so the test exercises the
    // submit-time stripping, not live type detection.
    state.creation_dialog.title_input = word.to_string();
    state.creation_dialog.cursor = word.chars().count();
    state.submit_creation_dialog().expect("note created");
    state
        .selected_item()
        .expect("item inserted")
        .title
        .clone()
}

#[test]
fn submit_keeps_plain_words_intact() {
    // Regression: the old prefix list contained bare letters ("x", "h", "-",
    // ...), so "hello" became "ello" and "xylofon" became "ylofon".
    for word in ["hello", "xylofon", "zrušení zakázky", "+420peněz"] {
        assert_eq!(submit_title(word), word, "title {word:?} must survive submit");
    }
}

#[test]
fn submit_still_strips_real_prefixes() {
    assert_eq!(submit_title(". hello"), "hello");
    assert_eq!(submit_title("x hotovo"), "hotovo");
}

#[test]
fn apply_selected_type_never_touches_plain_words() {
    let mut state = SpaiNotesState::new(None);
    state.open_creation_dialog();
    // Direct assignment: "xylofon" is not a prefix; apply on a non-empty
    // input is a no-op (plan §1), so no letter may be trimmed.
    state.creation_dialog.title_input = "xylofon".to_string();
    state.creation_dialog.cursor = 7;
    state.creation_dialog.type_selection = 0;
    state.apply_selected_type();
    assert_eq!(state.creation_dialog.title_input, "xylofon", "input untouched");
}

#[test]
fn arrow_type_cycling_is_gated_on_empty_input() {
    let mut state = SpaiNotesState::new(None);
    state.open_creation_dialog();
    state.creation_dialog.title_input = ". hello".to_string();

    let before = state.creation_dialog.type_selection;
    state.next_type();
    state.prev_type();
    assert_eq!(state.creation_dialog.type_selection, before, "↑↓ ignored while typing");

    state.creation_dialog.title_input.clear();
    state.next_type();
    assert_eq!(state.creation_dialog.type_selection, (before + 1) % SPAI_TYPE_OPTIONS.len());
}

