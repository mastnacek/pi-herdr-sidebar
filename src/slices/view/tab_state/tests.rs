//! Face-persistence tests: round-trip, defaults, and never guessing.

use super::*;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pi-sidebar-tab-{}-{}-{tag}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn a_saved_face_comes_back() {
    let dir = temp_dir("roundtrip");
    assert_eq!(load(Some(&dir)), DEFAULT, "nothing stored yet");

    for tab in [
        Tab::Zen,
        Tab::Skills,
        Tab::Mcp,
        Tab::Notes,
        Tab::Shortcuts,
        Tab::Status,
    ] {
        save(Some(&dir), tab);
        assert_eq!(load(Some(&dir)), tab, "{} did not round-trip", slug(tab));
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn slugs_are_stable_names_not_indices() {
    assert_eq!(slug(Tab::Status), "status");
    assert_eq!(
        from_slug("  STATUS\n"),
        Some(Tab::Status),
        "trimmed + case-insensitive"
    );
    assert_eq!(from_slug("9"), None, "an index is not a name");
    assert_eq!(from_slug(""), None);
    assert_eq!(from_slug("nonsense"), None);
}

#[test]
fn unusable_state_falls_back_to_status_instead_of_failing() {
    let dir = temp_dir("garbage");
    let path = dir.join(FILE_NAME);

    std::fs::write(&path, "nonsense").expect("write garbage");
    assert_eq!(load(Some(&dir)), Tab::Status);

    std::fs::write(&path, "").expect("write empty");
    assert_eq!(load(Some(&dir)), Tab::Status);

    // A state file that cannot be read at all (here: a directory).
    let as_dir = dir.join("blocked");
    std::fs::create_dir_all(as_dir.join(FILE_NAME)).expect("dir as file");
    assert_eq!(load(Some(&as_dir)), Tab::Status);

    // A missing state dir must not panic on save either.
    save(Some(&dir.join("does-not-exist-yet")), Tab::Skills);
    assert_eq!(load(Some(&dir.join("does-not-exist-yet"))), Tab::Skills);

    std::fs::remove_dir_all(&dir).ok();
}
