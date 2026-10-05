//! Disk-level helpers for SPAI note files: ID allocation, atomic writes.
//!
//! These keep every write crash-safe and every allocation collision-free even
//! when `pi-spai` (which maintains its own counter) writes into the same
//! folder at the same time.
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use super::note::SpaiNoteItem;

/// Extracts the numeric part of `SPAI-<digits>` from a string, if present.
fn spai_number(text: &str) -> Option<u32> {
    let idx = text.find("SPAI-")?;
    let rest = &text[idx + 5..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Computes the next free SPAI id number from the notes directory on disk.
///
/// Scans the `.md` filenames (so ids of files deleted from memory but still on
/// disk, or written by `pi-spai` since the last scan, are respected) and also
/// considers the currently loaded items. Returns `max + 1`; a fresh folder
/// yields `1`.
pub fn next_spai_number(spai_dir: &Path, items: &[SpaiNoteItem]) -> u32 {
    let mut max = 0u32;
    if let Ok(entries) = std::fs::read_dir(spai_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".md") {
                if let Some(n) = spai_number(&name) {
                    max = max.max(n);
                }
            }
        }
    }
    for item in items {
        if let Some(n) = spai_number(&item.id) {
            max = max.max(n);
        }
    }
    max + 1
}

/// Creates a new file that **must not exist yet**, failing on collision.
pub fn create_note_file(path: &Path, content: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("Soubor již existuje nebo nejde vytvořit: {}", e))?;
    file.write_all(content.as_bytes())
        .and_then(|_| file.flush())
        .map_err(|e| format!("Zápis selhal: {}", e))
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

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("spai_io_{}_{}", tag, nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn next_number_skips_gaps_left_by_deletions() {
        let dir = temp_dir("ids");
        for name in ["2026-01-01-SPAI-001-a.md", "2026-01-01-SPAI-003-b.md"] {
            std::fs::write(dir.join(name), "x").unwrap();
        }
        assert_eq!(next_spai_number(&dir, &[]), 4);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_note_file_refuses_to_overwrite() {
        let dir = temp_dir("create");
        let path = dir.join("note.md");
        create_note_file(&path, "first").unwrap();
        assert!(create_note_file(&path, "second").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn atomic_write_replaces_content_and_leaves_no_tmp() {
        let dir = temp_dir("atomic");
        let path = dir.join("note.md");
        std::fs::write(&path, "old").unwrap();
        write_atomic(&path, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert!(dir.read_dir().unwrap().count() == 1, "no .tmp leftover");
        std::fs::remove_dir_all(&dir).ok();
    }
}
