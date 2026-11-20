//! The project list from `spai projects`: it replaces the old discovery, a failure
//! keeps it, and order, selection and loaded notes survive the swap.
use super::state::SpaiNotesState;
use super::test_emulator::write_note;
use crate::shared::spai_client::testkit::{out, FakeRunner};
use std::path::{Path, PathBuf};

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("spai_pl_{tag}_{}_{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn project(root: &Path, name: &str, notes: usize) -> PathBuf {
    let path = root.join(name);
    for i in 0..notes {
        write_note(&path, name, &format!(". note {i}"));
    }
    path
}

fn answer(projects: &[(&str, &Path)]) -> Result<crate::shared::spai_client::RunOut, String> {
    let list: Vec<String> = projects
        .iter()
        .map(|(n, p)| {
            let path = p.to_string_lossy().replace('\\', "/");
            format!(
                r#"{{"name":"{n}","path":"{path}","records":1,"vectors":0,"sources":["scan"]}}"#
            )
        })
        .collect();
    let body = format!(
        r#"{{"ok":true,"data":{{"projects":[{}]}}}}"#,
        list.join(",")
    );
    out(0, &body, "")
}

/// Waits for the background `spai projects` and applies its answer.
fn settle(state: &mut SpaiNotesState) {
    while state.projects_rx.is_some() {
        state.poll_projects();
        std::thread::yield_now();
    }
}

/// A state whose old discovery found `cwd` first (the emulator has no `projects`;
/// the discovery also reads the real pi cache, so tests never count on the rest).
fn state_at(cwd: &Path) -> SpaiNotesState {
    let mut state = SpaiNotesState::new(Some(cwd.to_path_buf()));
    settle(&mut state);
    state
}

fn names(state: &SpaiNotesState) -> Vec<&str> {
    state.projects.iter().map(|p| p.name.as_str()).collect()
}

#[test]
fn list_from_spai_replaces_the_fallback() {
    let root = temp_dir("replace");
    let (a, b, c) = (
        project(&root, "a", 1),
        project(&root, "b", 1),
        project(&root, "c", 1),
    );
    let mut state = state_at(&a);
    assert_eq!(
        names(&state)[0],
        "a",
        "the old discovery shows the cwd project at once"
    );
    let (cfg, fake) = FakeRunner::config(vec![answer(&[("a", &a), ("b", &b), ("c", &c)])]);
    state.core = cfg;
    state.request_project_list();
    settle(&mut state);
    assert_eq!(names(&state), ["a", "b", "c"]);
    assert_eq!(fake.calls(), [["projects", "--json"]]);
    assert_eq!(
        state.current_items().len(),
        1,
        "notes of the selected project are loaded"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn failure_or_empty_answer_keeps_the_fallback_quietly() {
    let root = temp_dir("fail");
    let a = project(&root, "a", 1);
    let mut state = state_at(&a);
    let before: Vec<String> = names(&state).iter().map(|n| n.to_string()).collect();
    for reply in [
        Err(crate::shared::spai_client::NOT_FOUND.to_string()),
        out(1, r#"{"ok":false,"error":"boom"}"#, "boom"),
        out(0, "not json", ""),
        answer(&[]),
    ] {
        let (cfg, _) = FakeRunner::config(vec![reply]);
        state.core = cfg;
        state.request_project_list();
        settle(&mut state);
        assert_eq!(names(&state), before);
        assert!(state.status_message.is_none(), "nothing alarming is shown");
    }
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn order_selection_and_loaded_notes_survive_the_swap() {
    let root = temp_dir("keep");
    let (a, b, c) = (
        project(&root, "a", 3),
        project(&root, "b", 1),
        project(&root, "c", 1),
    );
    let gone = root.join("gone");
    let mut state = state_at(&a);
    state.next_item();
    state.next_item();
    let selected = state.selected_file_path().unwrap();
    let (cfg, _) = FakeRunner::config(vec![answer(&[
        ("c", &c),
        ("gone", &gone),
        ("a", &a),
        ("b", &b),
    ])]);
    state.core = cfg;
    state.request_project_list();
    settle(&mut state);
    assert_eq!(
        names(&state),
        ["c", "a", "b"],
        "spai's order; a folder without docs/spai is dropped"
    );
    assert_eq!(
        state.selected_project_idx, 1,
        "the selected project moved with its entry"
    );
    assert_eq!(state.selected_item_idx, 2);
    assert_eq!(state.selected_file_path().unwrap(), selected);
    state.selected_project_idx = 0;
    state.jump_to_active_project();
    assert_eq!(
        state.selected_project_idx, 1,
        "[p] still finds the active project"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_selected_project_spai_no_longer_lists_falls_back_to_the_first() {
    let root = temp_dir("lost");
    let (a, b) = (project(&root, "a", 1), project(&root, "b", 1));
    let mut state = state_at(&a);
    let (cfg, _) = FakeRunner::config(vec![answer(&[("b", &b)])]);
    state.core = cfg;
    state.request_project_list();
    settle(&mut state);
    assert_eq!(names(&state), ["b"]);
    assert_eq!(
        (state.selected_project_idx, state.selected_item_idx),
        (0, 0)
    );
    std::fs::remove_dir_all(&root).ok();
}
