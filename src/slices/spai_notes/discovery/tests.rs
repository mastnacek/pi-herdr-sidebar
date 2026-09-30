//! Discovery must stay cheap: probe directories, never read notes up front.
use super::*;
use std::fs;

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("spai_{tag}_{}_{}", std::process::id(), nanos));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn dir_has_notes_probes_without_reading() {
    let dir = temp_dir("probe");
    assert!(!dir_has_notes(&dir), "empty dir has no notes");
    assert!(
        !dir_has_notes(&dir.join("missing")),
        "missing dir has no notes"
    );

    fs::write(dir.join("README.txt"), "not a note").unwrap();
    assert!(!dir_has_notes(&dir), "non-markdown file is not a note");

    fs::write(dir.join(".hidden.md"), "hidden").unwrap();
    assert!(!dir_has_notes(&dir), "dotfile is not a note");

    fs::write(dir.join("2026-01-01-SPAI-001-a.md"), "x").unwrap();
    assert!(dir_has_notes(&dir), "markdown file is a note");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn project_notes_are_loaded_on_demand_only() {
    let project = temp_dir("lazy");
    let spai_dir = project.join("docs").join("spai");
    fs::create_dir_all(&spai_dir).unwrap();
    fs::write(
        spai_dir.join("2026-01-01-SPAI-001-a.md"),
        "---\ntype: Todo\ntitle: \"A\"\ntimestamp: 1\nstatus: todo\nsource: pi-spai\nspai_symbol: '.'\n---\n\n# SPAI-001: A\n. A\n",
    )
    .unwrap();

    let mut summary =
        SpaiProjectSummary::new("lazy".to_string(), project.clone(), spai_dir.clone());
    assert!(summary.items.is_empty(), "discovery must not read notes");

    summary.ensure_items();
    assert_eq!(summary.items.len(), 1, "first ensure_items loads them");

    fs::write(spai_dir.join("2026-01-02-SPAI-002-b.md"), "junk").unwrap();
    summary.ensure_items();
    assert_eq!(summary.items.len(), 1, "second ensure_items is a no-op");

    fs::remove_dir_all(&project).ok();
}

#[test]
fn fingerprint_is_none_for_missing_and_changes_with_content() {
    let dir = temp_dir("fp");
    assert_eq!(file_fingerprint(&dir.join("nope")), None);

    let file = dir.join("cache.json");
    fs::write(&file, "{}").unwrap();
    let first = file_fingerprint(&file).expect("fingerprint");
    assert_eq!(file_fingerprint(&file), Some(first), "stable while idle");

    fs::write(&file, "{\"projects\":[]}").unwrap();
    assert_ne!(
        file_fingerprint(&file),
        Some(first),
        "len change is detected"
    );

    fs::remove_dir_all(&dir).ok();
}
