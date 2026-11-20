//! Disk-level helper for SPAI note files: the atomic write used by the
//! editors (new records, status changes and deletes go through `spai`).
use crate::shared::spai_client::{self, CoreConfig};
use std::path::{Path, PathBuf};

/// Project root of a record file: `<root>/docs/spai/<file>.md` or `<root>/.pi/spai/<file>.md`.
pub fn project_of_file(file: &Path) -> Option<PathBuf> {
    Some(file.parent()?.parent()?.parent()?.to_path_buf())
}

/// Deletes the record `id` stored in `file` with `spai delete` (the sidebar
/// never removes record files itself).
pub fn delete_via_spai(core: &CoreConfig, file: &Path, id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("Záznam nemá ID, smazání přes spai není možné".to_string());
    }
    let project = project_of_file(file).ok_or_else(|| "Cesta záznamu nemá projekt".to_string())?;
    spai_client::delete_record(core, &project, id)
}

/// Crash-safe write: writes a sibling `.tmp` file, then renames it over the
/// target. An interrupted write leaves the old file intact.
pub fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, content).map_err(|e| format!("Zápis selhal: {}", e))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Přejmenování selhalo: {}", e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::spai_client::testkit::{out, FakeRunner};

    #[test]
    fn project_is_three_levels_above_the_file() {
        let file = Path::new("/p/proj/docs/spai/2026-01-01-SPAI-001-a.md");
        assert_eq!(project_of_file(file), Some(PathBuf::from("/p/proj")));
    }

    #[test]
    fn delete_calls_spai_delete_with_id_and_project() {
        let (cfg, fake) = FakeRunner::config(vec![out(0, r#"{"ok":true,"data":{}}"#, "")]);
        let file = Path::new("/p/proj/docs/spai/2026-01-01-SPAI-007-a.md");
        delete_via_spai(&cfg, file, "SPAI-007").unwrap();
        let calls = fake.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0][..2], ["delete", "SPAI-007"]);
        assert_eq!(calls[0][2], "--project");
        assert!(calls[0][3].replace('\\', "/").ends_with("/p/proj"), "{:?}", calls[0]);
        assert_eq!(calls[0].last().map(String::as_str), Some("--json"));
    }

    #[test]
    fn delete_failure_and_missing_id_are_errors_and_call_nothing_without_an_id() {
        let (cfg, fake) =
            FakeRunner::config(vec![out(1, r#"{"ok":false,"error":"record not found: 7"}"#, "")]);
        let file = Path::new("/p/proj/docs/spai/x.md");
        assert_eq!(delete_via_spai(&cfg, file, "7"), Err("record not found: 7".to_string()));
        assert!(delete_via_spai(&cfg, file, "").is_err());
        assert_eq!(fake.calls().len(), 1, "an empty id never reaches spai");
    }

    #[test]
    fn atomic_write_replaces_content_and_leaves_no_tmp() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("spai_io_atomic_{}", nanos));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("note.md");
        std::fs::write(&path, "old").unwrap();
        write_atomic(&path, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert!(dir.read_dir().unwrap().count() == 1, "no .tmp leftover");
        std::fs::remove_dir_all(&dir).ok();
    }
}
