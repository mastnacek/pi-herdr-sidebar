//! Creation dialog write through `spai`: background, duplicate, force, offline,
//! missing binary, status change.
use super::super::core_bridge::testkit::{duplicate, out, written, FakeRunner};
use super::super::core_bridge::{CoreConfig, ProcessRunner};
use super::*;
use std::fs;
use std::sync::Arc;

fn project(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("spai_add_{}_{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("docs").join("spai")).unwrap();
    dir
}

fn state_on(project: &std::path::Path) -> SpaiNotesState {
    let mut state = SpaiNotesState::new(Some(project.to_path_buf()));
    state.selected_project_idx = state
        .projects
        .iter()
        .position(|p| p.path == project)
        .expect("temp project discovered");
    state
}

fn type_text(state: &mut SpaiNotesState, text: &str) {
    state.open_creation_dialog();
    state.creation_dialog.title_input = text.to_string();
    state.creation_dialog.cursor = text.chars().count();
}

#[test]
fn write_runs_in_the_background_then_selects_the_new_record() {
    let p = project("bg");
    let mut state = state_on(&p);
    type_text(&mut state, ". Opravit build");
    state.submit_creation_dialog().unwrap();
    assert!(state.creation_dialog.saving);
    assert_eq!(state.status_message.as_deref(), Some("⏳ ukládám…"));
    state.wait_add_job();
    assert!(!state.creation_dialog.active, "dialog closed");
    assert!(!state.creation_dialog.saving);
    assert!(state.status_message.as_deref().unwrap().starts_with("✓ SPAI-001"));
    assert_eq!(state.selected_item().unwrap().id, "SPAI-001");
    fs::remove_dir_all(&p).ok();
}

#[test]
fn a_duplicate_shows_candidates_and_writes_nothing_until_save_anyway() {
    let p = project("dup");
    let mut state = state_on(&p);
    type_text(&mut state, ". DUPLIKAT věc");
    state.submit_creation_dialog().unwrap();
    state.wait_add_job();
    assert!(state.dedup.visible);
    assert_eq!(state.dedup.matches.len(), 1);
    assert!(state.dedup.matches[0].duplicate);
    assert_eq!(state.dedup.matches[0].project, "jiny");
    assert!(state.dedup.force_text.is_some());
    assert!(state.creation_dialog.active, "dialog stays open");
    assert!(!state.creation_dialog.saving);
    assert_eq!(fs::read_dir(p.join("docs").join("spai")).unwrap().count(), 0);

    state.force_creation_add().unwrap();
    state.wait_add_job();
    assert!(!state.creation_dialog.active);
    assert_eq!(fs::read_dir(p.join("docs").join("spai")).unwrap().count(), 1);
    assert!(!state.dedup.visible);
    fs::remove_dir_all(&p).ok();
}

#[test]
fn offline_write_shows_a_warning() {
    let p = project("offline");
    let mut state = state_on(&p);
    let (cfg, _) = FakeRunner::config(vec![written("SPAI-001", "alpha", "a.md", &["embed"])]);
    state.core = cfg;
    type_text(&mut state, ". x věc");
    state.submit_creation_dialog().unwrap();
    state.wait_add_job();
    let msg = state.status_message.clone().unwrap();
    assert!(msg.contains("bez sítě"), "{msg}");
    fs::remove_dir_all(&p).ok();
}

#[test]
fn missing_binary_keeps_the_dialog_and_explains() {
    let p = project("nobin");
    let mut state = state_on(&p);
    state.core = CoreConfig {
        runner: Arc::new(ProcessRunner { bin: None, env: Vec::new() }),
    };
    type_text(&mut state, ". x věc");
    state.submit_creation_dialog().unwrap();
    state.wait_add_job();
    assert!(state.creation_dialog.active, "text is not lost");
    assert!(!state.creation_dialog.saving);
    assert!(state.status_message.unwrap().contains("spai nenalezen"));
    fs::remove_dir_all(&p).ok();
}

#[test]
fn ctrl_d_asks_spai_similar_in_the_background() {
    let p = project("similar");
    let mut state = state_on(&p);
    let (cfg, fake) = FakeRunner::config(vec![duplicate("beta", "SPAI-005", "Stejné")]);
    state.core = cfg;
    state.run_dedup(". něco podobného".to_string());
    assert!(state.dedup.is_evaluating);
    while state.dedup.receiver.is_some() {
        state.poll_dedup_receiver();
        std::thread::yield_now();
    }
    assert_eq!(state.dedup.matches[0].id, "SPAI-005");
    assert_eq!(fake.calls()[0][0], "similar");
    fs::remove_dir_all(&p).ok();
}

#[test]
fn status_change_goes_through_spai_set_and_fails_cleanly() {
    let p = project("set");
    let mut state = state_on(&p);
    type_text(&mut state, ". úkol");
    state.submit_creation_dialog().unwrap();
    state.wait_add_job();
    state.cycle_selected_status().unwrap();
    assert_eq!(state.selected_item().unwrap().status.as_str(), "working");

    let (cfg, fake) = FakeRunner::config(vec![out(
        1,
        r#"{"ok":false,"error":"record not found: SPAI-001"}"#,
        "",
    )]);
    state.core = cfg;
    let err = state.cycle_selected_status().unwrap_err();
    assert_eq!(err, "record not found: SPAI-001");
    assert_eq!(state.selected_item().unwrap().status.as_str(), "working", "unchanged");
    assert_eq!(fake.calls()[0][..3], ["set", "SPAI-001", "waiting"]);
    fs::remove_dir_all(&p).ok();
}
