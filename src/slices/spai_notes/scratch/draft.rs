//! Draft autosave for unsaved Scratchpad lines (plan §7).
//!
//! Location: `HERDR_PLUGIN_STATE_DIR/scratch-draft.md` — never the source
//! tree. Only editable (New) record groups are stored; Saved lines live in
//! their project files. Groups are separated by a blank line.
use super::line_model::{is_marked, LineOrigin, ScratchLine};
use super::state::ScratchState;
use std::path::PathBuf;

pub fn draft_path(state_dir: Option<&PathBuf>) -> Option<PathBuf> {
    state_dir.as_ref().map(|d| d.join("scratch-draft.md"))
}

/// Writes the unsaved (New) record groups to the draft file.
pub fn save_draft(state: &ScratchState) {
    let Some(path) = draft_path(state.state_dir.as_ref()) else {
        return;
    };

    let mut groups: Vec<String> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut in_new_group = false;

    for line in &state.lines {
        match &line.origin {
            LineOrigin::New => {
                if is_marked(&line.text) && !current.is_empty() {
                    groups.push(current.join("\n"));
                    current.clear();
                }
                current.push(line.text.clone());
                in_new_group = true;
            }
            _ => {
                // Saved/FromFile lines break the group.
                if in_new_group {
                    if !current.is_empty() {
                        groups.push(current.join("\n"));
                        current.clear();
                    }
                    in_new_group = false;
                }
            }
        }
    }
    if !current.is_empty() {
        groups.push(current.join("\n"));
    }

    if groups.is_empty() {
        // Nothing unsaved: remove a stale draft.
        let _ = std::fs::remove_file(&path);
        return;
    }

    let content = groups.join("\n\n") + "\n";
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
    let _ = std::fs::write(&path, content);
}

/// Restores unsaved groups from the draft into `New` lines.
/// Returns the number of restored groups.
pub fn load_draft(state: &mut ScratchState) -> usize {
    let Some(path) = draft_path(state.state_dir.as_ref()) else {
        return 0;
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return 0;
    };

    let mut restored = 0;
    for group in content.split("\n\n") {
        let group = group.trim_end_matches('\n');
        if group.trim().is_empty() {
            continue;
        }
        for text in group.split('\n') {
            state.lines.push(ScratchLine {
                text: text.to_string(),
                origin: LineOrigin::New,
            });
        }
        restored += 1;
    }
    restored
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_state(tag: &str) -> ScratchState {
        let dir = std::env::temp_dir().join(format!("spai_draft_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = ScratchState::default();
        s.state_dir = Some(dir);
        s
    }

    #[test]
    fn draft_round_trips_unsaved_groups() {
        let mut state = temp_state("roundtrip");
        let mk = |t: &str| ScratchLine {
            text: t.to_string(),
            origin: LineOrigin::New,
        };
        state.lines = vec![
            mk(". First task"),
            mk("detail one"),
            mk("? Second idea"),
            ScratchLine {
                text: "x saved".to_string(),
                origin: LineOrigin::Saved {
                    path: PathBuf::from("/tmp/x.md"),
                    id: "SPAI-001".into(),
                    project: "p".into(),
                    saved_text: "x saved".into(),
                },
            },
        ];

        save_draft(&state);
        let mut restored = ScratchState::default();
        restored.state_dir = state.state_dir.clone();
        assert_eq!(load_draft(&mut restored), 2, "two unsaved groups");
        assert_eq!(restored.lines.len(), 3, "1+2 lines, saved line excluded");
        assert_eq!(restored.lines[0].text, ". First task");
        assert_eq!(restored.lines[1].text, "detail one");
        assert_eq!(restored.lines[2].text, "? Second idea");
        assert!(restored.lines.iter().all(|l| l.origin == LineOrigin::New));

        std::fs::remove_dir_all(state.state_dir.unwrap()).ok();
    }

    #[test]
    fn empty_draft_is_removed() {
        let mut state = temp_state("empty");
        let mk = |t: &str| ScratchLine {
            text: t.to_string(),
            origin: LineOrigin::Saved {
                path: PathBuf::from("/tmp/x.md"),
                id: "SPAI-001".into(),
                project: "p".into(),
                saved_text: t.into(),
            },
        };
        state.lines = vec![mk("x saved")];
        save_draft(&state);
        let path = draft_path(state.state_dir.as_ref()).unwrap();
        assert!(!path.exists(), "no stale draft when everything is saved");
        std::fs::remove_dir_all(state.state_dir.unwrap()).ok();
    }
}