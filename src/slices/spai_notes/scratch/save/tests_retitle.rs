//! Editing a loaded record's title: the Czech title (`title_cs`) is what the
//! user edits, the English one stays; `spai reindex` runs once after the save.
use super::super::state::ScratchScope;
use super::tests::{save_all, state_on, temp_project};
use crate::shared::spai_client::{CoreConfig, RunOut, Runner};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

struct Recorder {
    calls: Mutex<Vec<Vec<String>>>,
    code: i32,
}

impl Runner for Recorder {
    fn run(&self, args: &[String], _cwd: &Path) -> Result<RunOut, String> {
        self.calls.lock().unwrap().push(args.to_vec());
        Ok(RunOut {
            code: self.code,
            stdout: r#"{"ok":true,"data":{"totals":{"errors":0}}}"#.to_string(),
            stderr: String::new(),
        })
    }
}

const CS: &str = "---\ntype: Todo\ntitle: \"Fix login\"\ntimestamp: 2026-11-20 10:00:00\nstatus: todo\nsource: spai\nspai_symbol: '.'\nlang: en\ntitle_cs: \"Oprava přihlášení\"\n---\n\n# SPAI-001: Fix login\n\n. Fix login\n\nPoznámka\n";
const PLAIN: &str = "---\ntype: Todo\ntitle: \"Fix login\"\ntimestamp: 2026-11-20 10:00:00\nstatus: todo\nsource: spai\nspai_symbol: '.'\n---\n\n# SPAI-001: Fix login\n\n. Fix login\n";

/// A loaded record whose shown title is changed from `from` to `to`.
fn edit(content: &str, from: &str, to: &str, code: i32) -> (String, Arc<Recorder>, Vec<String>) {
    let dir = temp_project("retitle");
    let file: PathBuf = dir.join("docs/spai/2026-11-20-SPAI-001-fix-login.md");
    fs::write(&file, content).unwrap();
    let mut state = state_on(&dir);
    let rec = Arc::new(Recorder {
        calls: Mutex::new(Vec::new()),
        code,
    });
    state.core = CoreConfig {
        runner: rec.clone(),
    };
    state.switch_scratch_scope(ScratchScope::Project);
    let head = state.scratch.lines[0].text.clone();
    assert!(head.contains(from), "{head}");
    state.scratch.lines[0].text = head.replace(from, to);
    save_all(&mut state);
    let warnings = state.wait_reindex();
    let text = fs::read_to_string(&file).unwrap();
    fs::remove_dir_all(&dir).ok();
    (text, rec, warnings)
}

#[test]
fn edited_czech_title_keeps_english_title_and_asks_for_retitle() {
    let (text, rec, warnings) = edit(CS, "Oprava přihlášení", "Oprava loginu", 0);
    assert!(text.contains("title: \"Fix login\""), "{text}");
    assert!(text.contains("# SPAI-001: Fix login"), "{text}");
    assert!(text.contains("title_cs: \"Oprava loginu\""), "{text}");
    assert!(text.contains("pending: [retitle_en]"), "{text}");
    assert!(
        text.contains("\n. Fix login\n"),
        "SPAI line stays English: {text}"
    );
    assert!(text.contains("lang: en"), "{text}");
    let calls = rec.calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "reindex runs once");
    assert_eq!(calls[0][..2], ["reindex", "--project"]);
    assert_eq!(calls[0].last().map(String::as_str), Some("--json"));
    assert!(warnings.is_empty());
}

#[test]
fn unchanged_text_changes_nothing_and_runs_nothing() {
    let (text, rec, _) = edit(CS, "Oprava přihlášení", "Oprava přihlášení", 0);
    assert!(
        text.contains("title: \"Fix login\"") && text.contains("title_cs: \"Oprava přihlášení\"")
    );
    assert!(!text.contains("retitle_en"), "{text}");
    assert!(
        text.contains("\n. Fix login\n"),
        "SPAI line stays English: {text}"
    );
    assert!(rec.calls.lock().unwrap().is_empty());
}

#[test]
fn record_without_title_cs_changes_the_english_title_as_before() {
    let (text, rec, _) = edit(PLAIN, "Fix login", "Fix sign-in", 0);
    assert!(text.contains("title: \"Fix sign-in\""), "{text}");
    assert!(
        !text.contains("retitle_en") && !text.contains("title_cs"),
        "{text}"
    );
    assert!(rec.calls.lock().unwrap().is_empty());
}

#[test]
fn failing_reindex_is_only_a_warning_and_the_record_keeps_pending() {
    let (text, _, warnings) = edit(CS, "Oprava přihlášení", "Oprava loginu", 1);
    assert!(text.contains("pending: [retitle_en]"), "{text}");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

const BODY: &str = "---\ntype: Todo\ntitle: \"Fix login\"\ntimestamp: 2026-11-20 10:00:00\nstatus: todo\nsource: spai\nspai_symbol: '.'\nlang: en\ntitle_cs: \"Oprava přihlášení\"\n---\n\n# SPAI-001: Fix login\n\n. Fix login !high\n\n## Plan\n\n- step one\n- step two\n\nNotes below.\n";

fn loaded(content: &str, from: &str, to: &str) -> (String, usize) {
    let dir = temp_project("body");
    let file: PathBuf = dir.join("docs/spai/2026-11-20-SPAI-001-fix-login.md");
    fs::write(&file, content).unwrap();
    let mut state = state_on(&dir);
    state.core = CoreConfig {
        runner: Arc::new(Recorder {
            calls: Mutex::new(Vec::new()),
            code: 0,
        }),
    };
    state.switch_scratch_scope(ScratchScope::Project);
    let head = state.scratch.lines[0].text.clone();
    state.scratch.lines[0].text = head.replace(from, to);
    save_all(&mut state);
    state.wait_reindex();
    let text = fs::read_to_string(&file).unwrap();
    fs::remove_dir_all(&dir).ok();
    (text, state.scratch.last_batch.len())
}

#[test]
fn unchanged_loaded_record_is_not_written_at_all() {
    let (text, created) = loaded(BODY, "Oprava", "Oprava");
    assert_eq!(text, BODY, "byte for byte");
    assert_eq!(created, 0);
}

#[test]
fn changed_line_keeps_the_whole_body_below_it() {
    let (text, _) = loaded(BODY, "Oprava přihlášení", "Oprava loginu");
    for needle in [
        "## Plan",
        "- step one",
        "- step two",
        "Notes below.",
        ". Fix login !high",
        "title_cs: \"Oprava loginu\"",
    ] {
        assert!(text.contains(needle), "{needle} lost:\n{text}");
    }
    let plain = BODY.replace("title_cs: \"Oprava přihlášení\"\n", "");
    let (text, _) = loaded(&plain, "Fix login", "Fix sign-in");
    assert!(text.contains("title: \"Fix sign-in\""), "{text}");
    for needle in ["## Plan", "- step one", "Notes below."] {
        assert!(text.contains(needle), "{needle} lost:\n{text}");
    }
}
