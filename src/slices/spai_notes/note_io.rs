//! Disk-level helper for SPAI note files: the atomic write used by the
//! editors (new records and status changes go through `spai`).
use std::path::Path;

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
