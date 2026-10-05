//! Disk-backed Scratchpad save tests (plan verification list).
use super::super::super::state::SpaiNotesState;
use super::super::line_model::LineOrigin;
use std::fs;
use std::path::PathBuf;

fn temp_project(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("spai_scratch_{}_{}", tag, nanos));
    fs::create_dir_all(dir.join("docs").join("spai")).unwrap();
    dir
}

fn state_on(project: &PathBuf) -> SpaiNotesState {
    let mut state = SpaiNotesState::new(Some(project.clone()));
    let idx = state
        .projects
        .iter()
        .position(|p| p.path == *project)
        .expect("temp project discovered");
    state.selected_project_idx = idx;
    state
}

fn add_line(state: &mut SpaiNotesState, text: &str) {
    state.scratch.lines.push(super::super::line_model::ScratchLine {
        text: text.to_string(),
        origin: LineOrigin::New,
    });
}

#[test]
fn routes_at_mention_to_the_right_folder_and_falls_back_to_current() {
    let a = temp_project("route-a");
    let b = temp_project("route-b");
    let mut state = SpaiNotesState::new(None);
    // Register both projects manually (discovery is cache-driven).
    state.projects = vec![
        super::super::super::discovery::SpaiProjectSummary::new(
            "proj-a".to_string(),
            a.clone(),
            a.join("docs").join("spai"),
        ),
        super::super::super::discovery::SpaiProjectSummary::new(
            "proj-b".to_string(),
            b.clone(),
            b.join("docs").join("spai"),
        ),
    ];
    state.current_project_path = Some(a.clone());
    state.scratch.state_dir = None;

    add_line(&mut state, ". Fix build @proj-b");
    add_line(&mut state, "? Nápad bez zmínky");
    let n = state.scratch_save_all();
    assert_eq!(n, 2);

    // The @mention routed to proj-b, the fallback went to the current (a).
    assert!(
        b.join("docs").join("spai").read_dir().unwrap().count() >= 1,
        "@proj-b line landed in proj-b"
    );
    assert!(
        a.join("docs").join("spai").read_dir().unwrap().count() >= 1,
        "unrouted line fell back to the current project"
    );

    // Both lines are now Saved.
    assert!(state.scratch.lines.iter().all(|l| !l.origin.is_editable()));

    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn unknown_mention_leaves_the_line_in_the_scratchpad() {
    let a = temp_project("unknown");
    let mut state = state_on(&a);
    add_line(&mut state, ". Fix @neexistuje");

    let n = state.scratch_save_all();
    assert_eq!(n, 0, "nothing saved");
    let line = &state.scratch.lines[0];
    assert!(line.origin.is_editable(), "line stays (nothing is lost)");
    assert!(
        state
            .scratch
            .last_summary
            .as_deref()
            .unwrap_or("")
            .contains("1 chyb"),
        "error reported: {:?}",
        state.scratch.last_summary
    );
    fs::remove_dir_all(&a).ok();
}

#[test]
fn ids_are_unique_across_a_batch_of_three() {
    let a = temp_project("batch3");
    let mut state = state_on(&a);
    add_line(&mut state, ". První");
    add_line(&mut state, ". Druhá");
    add_line(&mut state, ". Třetí");

    let n = state.scratch_save_all();
    assert_eq!(n, 3);

    let mut ids: Vec<String> = Vec::new();
    for entry in fs::read_dir(a.join("docs").join("spai")).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(pos) = name.find("SPAI-") {
            ids.push(name[pos..pos + 8].to_string());
        }
    }
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 3, "unique ids in one batch: {ids:?}");

    // Summary names all three.
    let summary = state.scratch.last_summary.clone().unwrap();
    assert!(summary.contains("Uloženo 3"), "summary: {summary}");
    fs::remove_dir_all(&a).ok();
}

#[test]
fn partial_failure_does_not_destroy_the_rest() {
    let a = temp_project("partial");
    let mut state = state_on(&a);
    // The discovered project carries the temp dir name; mention it exactly.
    let proj_name = state.projects[state.selected_project_idx].name.clone();
    add_line(&mut state, format!(". Dobrý záznam @{}", proj_name).as_str());
    add_line(&mut state, ". Špatný cíl @neexistuje");

    let n = state.scratch_save_all();
    assert_eq!(n, 1, "the good line saved");
    // The bad line is still editable New.
    let bad = state
        .scratch
        .lines
        .iter()
        .find(|l| l.text.contains("Špatný"))
        .unwrap();
    assert!(bad.origin.is_editable(), "failed line stays editable");
    // The good line is Saved with a label.
    let good = state
        .scratch
        .lines
        .iter()
        .find(|l| l.text.contains("Dobrý"))
        .unwrap();
    assert!(!good.origin.is_editable());
    fs::remove_dir_all(&a).ok();
}

#[test]
fn undo_batch_deletes_the_files_and_restores_lines() {
    let a = temp_project("undo");
    let mut state = state_on(&a);
    add_line(&mut state, ". Vratný záznam");
    add_line(&mut state, "pokračování");
    assert_eq!(state.scratch_save_all(), 1);
    let file = state.scratch.last_batch[0].clone();
    assert!(file.exists());

    let n = state.scratch_undo_batch().unwrap();
    assert_eq!(n, 1);
    assert!(!file.exists(), "file deleted");
    // The line is back to New with the original text (group restored).
    assert_eq!(state.scratch.lines.len(), 2, "continuation restored");
    assert_eq!(state.scratch.lines[0].text, ". Vratný záznam");
    assert_eq!(state.scratch.lines[1].text, "pokračování");
    assert!(state.scratch.lines[0].origin.is_editable());
    fs::remove_dir_all(&a).ok();
}

#[test]
fn saved_lines_are_not_editable_and_ctrl_d_is_manual_only() {
    let a = temp_project("readonly");
    let mut state = state_on(&a);
    add_line(&mut state, ". Uložený záznam");
    assert_eq!(state.scratch_save_all(), 1);

    // Typing on a Saved line is rejected.
    assert!(!state.scratch.insert_char('X'), "Saved line rejects edits");
    // The fresh dedup panel makes the next Ctrl+S warn (no prompt).
    state.scratch.dedup.visible = true;
    state.scratch.dedup.matches.push(super::super::super::similarity::SimilarNoteMatch {
        id: "SPAI-009".to_string(),
        title: "jiny".to_string(),
        symbol: ".".to_string(),
        similarity: 0.9,
        is_vector_match: false,
    });
    add_line(&mut state, "? Nový řádek");
    state.scratch_save_all();
    assert!(
        state
            .scratch
            .warn_similar
            .as_deref()
            .unwrap_or("")
            .contains("SPAI-009"),
        "footer warning present: {:?}",
        state.scratch.warn_similar
    );
    fs::remove_dir_all(&a).ok();
}