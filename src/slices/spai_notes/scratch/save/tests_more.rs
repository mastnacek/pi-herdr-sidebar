//! Scratchpad save tests, part 2 (undo, duplicates, background write).
use super::super::line_model::LineOrigin;
use super::tests::*;
use std::fs;

#[test]
fn loaded_scope_cursor_starts_on_first_record() {
    let a = temp_project("cursor");
    let mut state = state_on(&a);
    for t in ["prvni", "druhy", "treti"] {
        state
            .create_quick_note_with_status(
                t,
                super::super::super::note::SpaiType::Todo,
                super::super::super::note::SpaiStatus::Todo,
                &format!(
                    ". {}
",
                    t
                ),
            )
            .unwrap();
    }
    state.switch_scratch_scope(super::super::state::ScratchScope::Project);
    // Cursor on the FIRST record (not the trailing typing slot).
    assert_eq!(state.scratch.cursor_line, 0);
    // Arrow down walks into the records; scrolling keeps the cursor visible.
    for i in 1..4 {
        state.scratch.cursor_down();
        assert_eq!(state.scratch.cursor_line, i);
    }
    // The visible window contains the cursor.
    let (scroll, h) =
        super::super::view::visible_window(state.scratch.lines.len(), state.scratch.cursor_line, 5);
    assert!(state.scratch.cursor_line >= scroll);
    assert!(state.scratch.cursor_line < scroll + h);
    fs::remove_dir_all(&a).ok();
}

#[test]
fn filter_is_live_only_matches_stay_visible() {
    let a = temp_project("live");
    let mut state = state_on(&a);
    for t in ["build", "review", "build again"] {
        state
            .create_quick_note_with_status(
                t,
                super::super::super::note::SpaiType::Todo,
                super::super::super::note::SpaiStatus::Todo,
                &format!(
                    ". {}
",
                    t
                ),
            )
            .unwrap();
    }
    state.switch_scratch_scope(super::super::state::ScratchScope::Project);

    // Simulate typing into the filter input: live application per keystroke.
    state.scratch.input_mode = Some(super::super::state::ScratchInput::FuzzyFilter);
    for c in "buil".chars() {
        state.scratch.input_buffer.push(c);
        let text = state.scratch.input_buffer.clone();
        state.scratch.apply_filter_text(Some(text));
    }
    // "review" has no 'b' → hidden; both "build…" records match the
    // subsequence → visible.
    let count = state.scratch.visible_count();
    assert_eq!(count, 2, "review is hidden by 'buil'");

    // Typing 'rev' hides both build records, shows only "review".
    state.scratch.input_buffer.clear();
    state.scratch.apply_filter_text(Some("rev".to_string()));
    assert_eq!(state.scratch.visible_count(), 1);

    // Esc cancels: the snapshot filter (None) is restored → all visible.
    state.scratch.input_buffer.clear();
    state.scratch.filter = state.scratch.filter_before_input.take();
    assert_eq!(state.scratch.visible_count(), 3);
    fs::remove_dir_all(&a).ok();
}

#[test]
fn semantic_allow_set_survives_buffer_edits_no_shift() {
    let a = temp_project("semshift");
    let mut state = state_on(&a);
    for t in ["build", "review", "docs"] {
        state
            .create_quick_note_with_status(
                t,
                super::super::super::note::SpaiType::Todo,
                super::super::super::note::SpaiStatus::Todo,
                &format!(
                    ". {}
",
                    t
                ),
            )
            .unwrap();
    }
    state.switch_scratch_scope(super::super::state::ScratchScope::Project);
    // Simulate a semantic result: only the "review" record allowed (path-keyed).
    let file = state.projects[state.selected_project_idx]
        .items
        .iter()
        .find(|it| it.title.contains("review"))
        .unwrap()
        .file_path
        .clone();
    let mut allowed = std::collections::HashSet::new();
    allowed.insert(file);
    state.scratch.semantic_allowed = Some(allowed);

    // The user types a continuation line on the FIRST record — indices shift,
    // the allow-set must not.
    state.scratch.cursor_line = 0;
    state.scratch.split_line();
    assert_eq!(
        state.scratch.visible_count(),
        1,
        "only review stays visible"
    );
    // The review record is still findable by text in the buffer.
    assert!(
        state
            .scratch
            .lines
            .iter()
            .any(|l| l.text.contains("review")),
        "review record still in the buffer"
    );
    fs::remove_dir_all(&a).ok();
}

#[test]
fn semantic_keep_continuation_lines_with_their_record() {
    let a = temp_project("semcont");
    let mut state = state_on(&a);
    state
        .create_quick_note_with_status(
            "dlouhy",
            super::super::super::note::SpaiType::Todo,
            super::super::super::note::SpaiStatus::Todo,
            ". dlouhy zaznam
pokračovací řádek
",
        )
        .unwrap();
    state.switch_scratch_scope(super::super::state::ScratchScope::Project);
    // Semantic result allows only the "dlouhy" record.
    let file = state.projects[state.selected_project_idx].items[0]
        .file_path
        .clone();
    let mut allowed = std::collections::HashSet::new();
    allowed.insert(file);
    state.scratch.semantic_allowed = Some(allowed);
    state.scratch.cursor_line = 0;

    // Buffer layout after scope load: [". SPAI-001 dlouhy", "pokračovací…", ""].
    // The continuation line must NOT be judged as its own record — only the
    // group verdict counts.
    assert!(!state.scratch.record_hidden(0), "head visible");
    assert!(
        !state.scratch.record_hidden(1),
        "continuation stays with its record"
    );
    fs::remove_dir_all(&a).ok();
}

#[test]
fn navigation_skips_hidden_records_no_popin() {
    let a = temp_project("navskip");
    let mut state = state_on(&a);
    for t in ["jedna", "dva", "tri", "ctyri"] {
        state
            .create_quick_note_with_status(
                t,
                super::super::super::note::SpaiType::Todo,
                super::super::super::note::SpaiStatus::Todo,
                &format!(
                    ". {}
",
                    t
                ),
            )
            .unwrap();
    }
    state.switch_scratch_scope(super::super::state::ScratchScope::Project);
    // Semantic filter allows only records "dva" and "ctyri" (by path).
    let paths: Vec<_> = state.projects[state.selected_project_idx]
        .items
        .iter()
        .filter(|it| it.title.contains("dva") || it.title.contains("ctyri"))
        .map(|it| it.file_path.clone())
        .collect();
    let mut allowed = std::collections::HashSet::new();
    for p in paths {
        allowed.insert(p);
    }
    state.scratch.semantic_allowed = Some(allowed);
    state.scratch.cursor_line = 0;
    assert_eq!(state.scratch.visible_count(), 2);

    // What is visible and what hidden?
    let vis: Vec<String> = state
        .scratch
        .lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            format!(
                "{}:{}:{}",
                i,
                l.text,
                if state.scratch.record_hidden(i) {
                    "H"
                } else {
                    "V"
                }
            )
        })
        .collect();
    eprintln!("VIS={vis:?}");

    // (The Project scope loads records in descending id order: 004, 003,
    // 002, 001 — visible are "ctyri" (line 0) and "dva" (line 2).)
    assert!(!state.scratch.record_hidden(0));
    assert!(state.scratch.record_hidden(1));

    // ↓ jumps OVER the hidden record straight to the next VISIBLE one —
    // no pop-in of hidden entries while navigating.
    state.scratch.cursor_step_visible(1);
    assert!(
        state.scratch.lines[state.scratch.cursor_line]
            .text
            .contains("dva"),
        "landed on the next VISIBLE record, skipping the hidden one"
    );
    // ↑ back; clamped at the edges.
    state.scratch.cursor_step_visible(-1);
    assert!(state.scratch.lines[state.scratch.cursor_line]
        .text
        .contains("ctyri"));
    state.scratch.cursor_step_visible(-5);
    assert!(state.scratch.lines[state.scratch.cursor_line]
        .text
        .contains("ctyri"));
    fs::remove_dir_all(&a).ok();
}


#[test]
fn a_duplicate_stops_the_write_and_save_anyway_writes_it() {
    let a = temp_project("dupl");
    let mut state = state_on(&a);
    state.scratch.open = true;
    add_line(&mut state, ". DUPLIKAT záznam");
    state.scratch_save_all();
    assert!(state.scratch.save_job.is_some(), "the write runs in the background");
    assert_eq!(state.scratch.last_summary.as_deref(), Some("⏳ ukládám…"));
    state.wait_scratch_save();
    assert!(state.scratch.last_batch.is_empty(), "nothing written");
    assert!(state.scratch.dedup.visible);
    assert_eq!(state.scratch.dedup.matches[0].id, "SPAI-099");
    assert!(state.scratch.last_summary.as_deref().unwrap().contains("duplicita"));
    assert!(matches!(state.scratch.lines[0].origin, LineOrigin::New));
    assert_eq!(fs::read_dir(a.join("docs").join("spai")).unwrap().count(), 0);

    state.scratch_force_save().unwrap();
    state.wait_scratch_save();
    assert_eq!(state.scratch.last_batch.len(), 1, "saved anyway");
    assert!(matches!(state.scratch.lines[0].origin, LineOrigin::Saved { .. }));
    assert!(!state.scratch.dedup.visible);
    fs::remove_dir_all(&a).ok();
}

#[test]
fn the_inline_stamp_is_not_sent_to_spai() {
    let a = temp_project("stampadd");
    let mut state = state_on(&a);
    state.scratch.open = true;
    add_line(&mut state, ". [10.2.2026 14:22:37] Opravit build");
    assert_eq!(save_all(&mut state), 1);
    let content = fs::read_to_string(&state.scratch.last_batch[0]).unwrap();
    assert!(content.contains("# SPAI-001: Opravit build"), "{content}");
    assert!(!content.contains("10.2.2026"), "{content}");
    fs::remove_dir_all(&a).ok();
}
