//! Unit tests for note lifecycle actions: status cycles, deletion, creation.
use super::*;
use crate::slices::spai_notes::discovery::SpaiProjectSummary;
use std::fs;

fn temp_project() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("spai_del_test_{}_{}", std::process::id(), nanos));
    fs::create_dir_all(dir.join("docs").join("spai")).unwrap();
    dir
}

fn write_note(project: &std::path::Path, id: u32, title: &str) -> std::path::PathBuf {
    let path = project
        .join("docs")
        .join("spai")
        .join(format!("2026-01-01-SPAI-{id:03}-{title}.md"));
    fs::write(
        &path,
        format!(
            "---\ntype: Todo\ntitle: \"{title}\"\ntimestamp: 1\nstatus: todo\nsource: pi-spai\nspai_symbol: '.'\n---\n\n# SPAI-{id:03}: {title}\n. {title}\n"
        ),
    )
    .unwrap();
    path
}

fn state_on(project: &std::path::Path) -> SpaiNotesState {
    let mut state = SpaiNotesState::new(Some(project.to_path_buf()));
    let idx = state
        .projects
        .iter()
        .position(|p| p.path == project)
        .expect("temp project discovered");
    state.selected_project_idx = idx;
    state
}

#[test]
fn delete_removes_file_from_disk_and_drops_the_item() {
    let project = temp_project();
    let keep = write_note(&project, 1, "keep");
    let gone = write_note(&project, 2, "gone");
    let mut state = state_on(&project);
    assert_eq!(state.current_items().len(), 2);

    state.selected_item_idx = 1;
    state.begin_delete_selected();
    assert!(state.delete_confirm_active, "first Delete arms the confirm");

    state.confirm_delete_selected().unwrap();

    assert!(!gone.exists(), "selected note file must be gone from disk");
    assert!(keep.exists(), "other note files must survive");
    assert!(!state.delete_confirm_active);
    let titles: Vec<&str> = state
        .current_items()
        .iter()
        .map(|i| i.title.as_str())
        .collect();
    assert!(!titles.contains(&"gone"), "item removed from state: {titles:?}");

    fs::remove_dir_all(&project).ok();
}

#[test]
fn cancelled_delete_keeps_everything() {
    let project = temp_project();
    let note = write_note(&project, 1, "safe");
    let mut state = state_on(&project);

    state.begin_delete_selected();
    assert!(state.delete_confirm_active);
    state.cancel_delete();

    assert!(note.exists(), "cancel must keep the file on disk");
    assert_eq!(state.current_items().len(), 1);
    assert!(!state.delete_confirm_active);

    fs::remove_dir_all(&project).ok();
}

#[test]
fn create_never_reuses_an_id_deleted_from_disk() {
    let project = temp_project();
    let _a = write_note(&project, 1, "a");
    let _b = write_note(&project, 2, "b");
    let _c = write_note(&project, 3, "c");
    let mut state = state_on(&project);

    // Delete SPAI-002 (index 1), then create a new note.
    state.selected_item_idx = 1;
    state.confirm_delete_selected().unwrap();
    let new_id = state.create_quick_note_with_status("new", SpaiType::Todo, SpaiStatus::Todo, ". new
").unwrap();

    assert_ne!(new_id, "SPAI-002", "deleted id must not be reused");
    assert_eq!(new_id, "SPAI-004", "id continues from the disk max");
    let spai = project.join("docs").join("spai");
    let created = fs::read_dir(&spai)
        .unwrap()
        .flatten()
        .find(|e| e.file_name().to_string_lossy().contains("SPAI-004"))
        .expect("a file named *SPAI-004* must exist");
    let on_disk = fs::read_to_string(created.path()).unwrap();
    assert!(on_disk.contains("# SPAI-004: new"), "header in file: {on_disk}");
    assert!(on_disk.contains(". new"), "body in file: {on_disk}");
    assert_eq!(state.current_items().len(), 3);

    fs::remove_dir_all(&project).ok();
}

#[test]
fn create_recreates_a_missing_spai_dir() {
    let project = temp_project();
    // Discover while the folder exists, then remove it behind the state's
    // back — create must rebuild it instead of failing with a raw error.
    let mut state = state_on(&project);
    fs::remove_dir_all(project.join("docs").join("spai")).unwrap();

    let id = state.create_quick_note_with_status("prvni", SpaiType::Todo, SpaiStatus::Todo, ". prvni
").unwrap();
    assert_eq!(id, "SPAI-001");
    let created = fs::read_dir(project.join("docs").join("spai"))
        .expect("docs/spai must be recreated")
        .flatten()
        .count();
    assert_eq!(created, 1);

    fs::remove_dir_all(&project).ok();
}

#[test]
fn delete_on_empty_list_is_a_no_op_message() {
    let project = temp_project();
    let mut state = state_on(&project);
    assert!(state.current_items().is_empty());

    state.begin_delete_selected();
    assert!(!state.delete_confirm_active, "nothing to delete, no arming");
    assert!(state.status_message.is_some());

    fs::remove_dir_all(&project).ok();
}
