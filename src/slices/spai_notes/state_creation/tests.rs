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
fn apply_selected_type_inserts_and_replaces_prefix() {
    let mut state = SpaiNotesState::new(None);
    state.open_creation_dialog();

    // Select idea (?) which is index 5
    state.creation_dialog.type_selection = 5;
    state.apply_selected_type();
    assert_eq!(state.creation_dialog.title_input, "? ");

    // Type rest of text
    state.creation_dialog.title_input.push_str("novy napad");
    assert_eq!(state.creation_dialog.title_input, "? novy napad");

    // Switch to note (-) which is index 6 and apply
    state.creation_dialog.type_selection = 6;
    state.apply_selected_type();
    assert_eq!(state.creation_dialog.title_input, "- novy napad");
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
    assert_eq!(state.creation_dialog.type_selection, 7); // Critical Note
    assert_eq!(state.creation_dialog.selected_kind, SpaiType::Note);
}
