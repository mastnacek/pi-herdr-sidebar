//! Project filter (`/`) and project switching that keeps its place.
use super::state::SpaiNotesState;
use super::test_emulator::write_note;
use crate::shared::spai_client::testkit::{out, FakeRunner};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::{Path, PathBuf};

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("prj_{tag}_{}_{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn project(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    write_note(&path, name, ". note");
    path
}

fn settle(state: &mut SpaiNotesState) {
    while state.projects_rx.is_some() {
        state.poll_projects();
        std::thread::yield_now();
    }
}

/// A state whose project list came from a scripted `spai projects` (names in that order).
fn state_with(root: &Path, names: &[&str]) -> SpaiNotesState {
    let paths: Vec<PathBuf> = names.iter().map(|n| project(root, n)).collect();
    let mut state = SpaiNotesState::new(Some(paths[0].clone()));
    settle(&mut state);
    let list: Vec<String> = names
        .iter()
        .zip(&paths)
        .map(|(n, p)| {
            let path = p.to_string_lossy().replace('\\', "/");
            format!(r#"{{"name":"{n}","path":"{path}","records":1,"vectors":0,"sources":["scan"]}}"#)
        })
        .collect();
    let body = format!(r#"{{"ok":true,"data":{{"projects":[{}]}}}}"#, list.join(","));
    let (cfg, _) = FakeRunner::config(vec![out(0, &body, "")]);
    state.core = cfg;
    state.request_project_list();
    settle(&mut state);
    state
}

fn selected(state: &SpaiNotesState) -> &str {
    &state.projects[state.selected_project_idx].name
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn type_str(state: &mut SpaiNotesState, text: &str) {
    for c in text.chars() {
        assert!(state.handle_project_filter_key(&key(KeyCode::Char(c))));
    }
}

#[test]
fn switching_then_refreshing_keeps_the_selection_and_list() {
    let root = temp_dir("switch");
    let mut state = state_with(&root, &["zeta", "alfa", "beta"]);
    let cwd = state.current_project_path.clone();
    state.next_project();
    assert_eq!(selected(&state), "alfa");
    for _ in 0..3 {
        state.refresh(cwd.as_deref(), false);
        settle(&mut state);
        assert_eq!(selected(&state), "alfa");
    }
    state.refresh(cwd.as_deref(), true);
    settle(&mut state);
    let names: Vec<_> = state.projects.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names.len(), 3, "spai's list is never replaced by discovery");
    assert_eq!(selected(&state), "alfa", "forced refresh keeps it too");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn right_and_left_move_exactly_one_project_in_both_lists() {
    let root = temp_dir("step");
    let mut state = state_with(&root, &["a", "b", "c", "d"]);
    let first = state.selected_project_idx;
    state.next_project();
    assert_eq!(state.selected_project_idx, (first + 1) % 4);
    state.next_project();
    state.prev_project();
    assert_eq!(state.selected_project_idx, (first + 1) % 4);
    state.prev_project();
    assert_eq!(state.selected_project_idx, first);
    // The fallback list (spai never answered) behaves the same.
    let cwd = state.projects[0].path.clone();
    let mut fallback = SpaiNotesState::new(Some(cwd));
    let n = fallback.projects.len();
    if n > 1 {
        let at = fallback.selected_project_idx;
        fallback.next_project();
        assert_eq!(fallback.selected_project_idx, (at + 1) % n);
        fallback.prev_project();
        assert_eq!(fallback.selected_project_idx, at);
    }
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn notes_change_reloads_items_without_touching_the_list() {
    let root = temp_dir("notes");
    let mut state = state_with(&root, &["a", "b"]);
    let cwd = state.current_project_path.clone();
    state.next_project();
    let before: Vec<_> = state.projects.iter().map(|p| p.path.clone()).collect();
    let idx = state.selected_project_idx;
    assert_eq!(state.current_items().len(), 1);
    let path = state.projects[idx].path.clone();
    std::thread::sleep(std::time::Duration::from_millis(20));
    write_note(&path, "b", ". second");
    state.refresh(cwd.as_deref(), false);
    assert!(state.projects_rx.is_none(), "no new `spai projects` call");
    assert_eq!(state.current_items().len(), 2);
    let after: Vec<_> = state.projects.iter().map(|p| p.path.clone()).collect();
    assert_eq!(before, after);
    assert_eq!(state.selected_project_idx, idx);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn filter_matches_case_diacritics_and_path() {
    let root = temp_dir("match");
    let mut state = state_with(&root, &["spai", "pi-spai", "herdr-spai", "spai-kanban", "Žluťoučký", "other"]);
    state.open_project_filter();
    type_str(&mut state, "SPA");
    let names: Vec<_> = state
        .project_filter_matches()
        .iter()
        .map(|&i| state.projects[i].name.clone())
        .collect();
    assert_eq!(names, ["spai", "pi-spai", "herdr-spai", "spai-kanban"]);
    for _ in 0..3 {
        state.handle_project_filter_key(&key(KeyCode::Backspace));
    }
    type_str(&mut state, "zlutoucky");
    assert_eq!(state.project_filter_matches().len(), 1, "diacritics ignored");
    state.handle_project_filter_key(&key(KeyCode::Backspace));
    state.project_filter.as_mut().unwrap().query = "prj_match".into();
    assert_eq!(state.project_filter_matches().len(), 6, "path matches");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn filter_navigates_among_matches_enter_and_esc() {
    let root = temp_dir("nav");
    let mut state = state_with(&root, &["spai", "x", "pi-spai", "y", "spai-kanban"]);
    let start = state.selected_project_idx;
    state.open_project_filter();
    type_str(&mut state, "spai");
    assert_eq!(selected(&state), "spai");
    state.handle_project_filter_key(&key(KeyCode::Right));
    assert_eq!(selected(&state), "pi-spai");
    state.handle_project_filter_key(&key(KeyCode::Down));
    assert_eq!(selected(&state), "spai-kanban");
    state.handle_project_filter_key(&key(KeyCode::Right));
    assert_eq!(selected(&state), "spai", "wraps among matches");
    state.handle_project_filter_key(&key(KeyCode::Left));
    assert_eq!(selected(&state), "spai-kanban");
    state.handle_project_filter_key(&key(KeyCode::Enter));
    assert!(state.project_filter.is_none());
    assert_eq!(selected(&state), "spai-kanban", "Enter keeps the choice");
    // Esc returns to the project selected before.
    state.select_project(start);
    state.open_project_filter();
    type_str(&mut state, "kanban");
    assert_eq!(selected(&state), "spai-kanban");
    state.handle_project_filter_key(&key(KeyCode::Esc));
    assert!(state.project_filter.is_none());
    assert_eq!(state.selected_project_idx, start);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn no_match_keeps_the_selection_and_keys_do_not_leak() {
    let root = temp_dir("none");
    let mut state = state_with(&root, &["a", "b"]);
    state.next_project();
    let at = state.selected_project_idx;
    assert!(!state.handle_project_filter_key(&key(KeyCode::Char('q'))), "closed: not consumed");
    state.open_project_filter();
    type_str(&mut state, "zzz");
    assert!(state.project_filter_matches().is_empty());
    assert_eq!(state.selected_project_idx, at);
    state.handle_project_filter_key(&key(KeyCode::Right));
    assert_eq!(state.selected_project_idx, at, "no matches: arrows do nothing");
    for code in [KeyCode::Char('q'), KeyCode::Char('n'), KeyCode::Char('/'), KeyCode::Tab, KeyCode::Delete] {
        assert!(state.handle_project_filter_key(&key(code)), "{code:?} is consumed");
    }
    assert!(state.project_filter.is_some(), "q and Tab did not close it");
    assert!(!state.creation_dialog.active);
    assert!(state.handle_project_filter_key(&key(KeyCode::Enter)));
    assert_eq!(state.selected_project_idx, at);
    std::fs::remove_dir_all(&root).ok();
}
