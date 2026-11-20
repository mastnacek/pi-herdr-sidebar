//! Czech body: `body_cs` shown, null/failure -> English, toggle, cache reuse.
use super::super::state::SpaiNotesState;
use super::super::test_emulator::write_note;
use crate::shared::spai_client::testkit::{out, FakeRunner};
use std::path::PathBuf;
use std::sync::Arc;

fn show(body_cs: &str) -> Result<crate::shared::spai_client::RunOut, String> {
    out(0, &format!(r#"{{"ok":true,"data":{{"body_cs":{body_cs},"body_cs_stale":false}}}}"#), "")
}

/// A state on a temp project with one record, `spai` replaced by `answers`.
fn state_with(
    answers: Vec<Result<crate::shared::spai_client::RunOut, String>>,
) -> (SpaiNotesState, Arc<FakeRunner>, PathBuf) {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!("spai_bcs_{}_{nanos}", std::process::id()));
    write_note(&root, "p", ". Task\n\nEnglish body");
    let mut state = SpaiNotesState::new(Some(root.clone()));
    let (cfg, fake) = FakeRunner::config(answers);
    state.core = cfg;
    let at = state.projects.iter().position(|p| p.path == root).expect("project");
    state.select_project(at);
    (state, fake, root)
}

fn settle(state: &mut SpaiNotesState) {
    state.poll_body_cs();
    while state.body_cs.pending_is_some() {
        state.poll_body_cs();
        std::thread::yield_now();
    }
}

fn shown(state: &SpaiNotesState) -> String {
    state.viewer_body(state.selected_item().unwrap()).to_string()
}

#[test]
fn english_at_once_czech_after_spai_answers() {
    let (mut state, fake, _) = state_with(vec![show(r#""Český text""#)]);
    assert!(shown(&state).contains("English body"));
    settle(&mut state);
    assert_eq!(shown(&state), "Český text");
    let call = &fake.calls()[0];
    assert_eq!(call[0], "show");
    assert!(call.contains(&"--json".to_string()));
}

#[test]
fn null_and_failure_keep_english() {
    let (mut state, _, _) = state_with(vec![show("null")]);
    settle(&mut state);
    assert!(shown(&state).contains("English body"));
    let (mut state, _, _) = state_with(vec![Err("spai nenalezen".to_string())]);
    settle(&mut state);
    assert!(shown(&state).contains("English body"));
}

#[test]
fn toggle_switches_to_english_and_back() {
    let (mut state, _, _) = state_with(vec![show(r#""Český text""#)]);
    settle(&mut state);
    state.toggle_body_lang();
    assert!(shown(&state).contains("English body"));
    state.toggle_body_lang();
    assert_eq!(shown(&state), "Český text");
}

#[test]
fn answer_is_cached_until_the_file_changes() {
    let (mut state, fake, _) = state_with(vec![show(r#""Český text""#)]);
    settle(&mut state);
    settle(&mut state);
    settle(&mut state);
    assert_eq!(fake.calls().len(), 1, "cached");
    let path = state.selected_item().unwrap().file_path.clone();
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("\nmore\n");
    std::fs::write(&path, text).unwrap();
    settle(&mut state);
    assert_eq!(fake.calls().len(), 2, "file changed: asked again");
}
