//! Scratchpad line model (plan §1, §6).
//!
//! One buffer line = at most one record. A **marked** line (SPAI prefix from
//! the shared table, or a bare mark symbol) starts a record; following
//! unmarked lines belong to it as continuation. Prose (unmarked) lines stay
//! in the scratchpad and are never saved.
//!
//! Editing is **bidirectional** (plan §2.4 as lived): every line is editable,
//! including Saved ones. A Saved record whose text changes is updated in its
//! file on the next Ctrl+S (same id, same file); `saved_text` keeps the
//! original so `u` (undo batch) can restore it.
use super::super::spai_prefixes::strip_leading_prefix;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct ScratchLine {
    pub text: String,
    pub origin: LineOrigin,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineOrigin {
    /// Typed in this session (or restored from the draft) — saves as a new file.
    New,
    /// Written to disk by a Ctrl+S batch; further edits update the file.
    /// `saved_text` keeps the original text of this line so `u` (undo batch)
    /// can restore it.
    Saved {
        path: PathBuf,
        id: String,
        project: String,
        saved_text: String,
    },
    /// A record loaded from a project (Project/All scope). These are summaries
    /// (`. SPAI-014 Title`), not the full record text — real editing goes
    /// through the Notes editor (Enter/o), so Ctrl+S skips them.
    FromFile {
        path: PathBuf,
        id: String,
        project: String,
    },
}

impl LineOrigin {
    pub fn file_path(&self) -> Option<&PathBuf> {
        match self {
            LineOrigin::New => None,
            LineOrigin::Saved { path, .. } | LineOrigin::FromFile { path, .. } => Some(path),
        }
    }

    pub fn record_id(&self) -> Option<&str> {
        match self {
            LineOrigin::New => None,
            LineOrigin::Saved { id, .. } | LineOrigin::FromFile { id, .. } => Some(id),
        }
    }

    pub fn project(&self) -> Option<&str> {
        match self {
            LineOrigin::New => None,
            LineOrigin::Saved { project, .. } | LineOrigin::FromFile { project, .. } => {
                Some(project)
            }
        }
    }

}

/// A line carrying a SPAI mark: a full table prefix (`. `, `x `, `!- `, …) or
/// a bare mark symbol as the whole line (`.` `/` `x` `?` `-` `#` `*` `%` `;`).
pub fn is_marked(line: &str) -> bool {
    if strip_leading_prefix(line).is_some() {
        return true;
    }
    matches!(
        line.trim(),
        "." | "/" | "x" | "X" | "z" | "Z" | "?" | "-" | "+" | "=" | "*" | "%" | "~" | "$" | "♥"
            | "h" | "H" | "#" | ";" | "!" | "!!"
    )
}

/// Indices of lines that start a record (line 0 always; any marked line after).
pub fn record_starts(lines: &[ScratchLine]) -> Vec<usize> {
    let mut starts = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i == 0 || is_marked(&line.text) {
            starts.push(i);
        }
    }
    starts
}

/// The record group containing `idx`: from its start to the next record start.
pub fn record_span(lines: &[ScratchLine], idx: usize) -> (usize, usize) {
    let starts = record_starts(lines);
    let start = starts.iter().rev().find(|&&s| s <= idx).copied().unwrap_or(0);
    let end = starts
        .iter()
        .find(|&&s| s > start)
        .copied()
        .unwrap_or(lines.len());
    (start, end)
}

/// The record's full text: marked first line + continuation lines.
pub fn record_text(lines: &[ScratchLine], idx: usize) -> String {
    let (start, end) = record_span(lines, idx);
    lines[start..end]
        .iter()
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_line(text: &str) -> ScratchLine {
        ScratchLine {
            text: text.to_string(),
            origin: LineOrigin::New,
        }
    }

    #[test]
    fn marked_lines_use_the_shared_table_and_bare_symbols() {
        assert!(is_marked(". Fix build @herdr"));
        assert!(is_marked("/. čekám na review"));
        assert!(is_marked("!- kritická poznámka"));
        assert!(is_marked("?"));
        assert!(is_marked(";"));
        assert!(is_marked("-"));
        assert!(!is_marked("hello")); // prose
        assert!(!is_marked("")); // empty
        assert!(!is_marked("-0.5% ztráta")); // not a prefix (no space)
    }

    #[test]
    fn records_group_marked_line_with_continuations() {
        let lines = vec![
            new_line("prose intro"),
            new_line(". Fix build @herdr"),
            new_line("detail jeden"),
            new_line("detail dva"),
            new_line("? Nový nápad"),
            new_line("plain line"), // belongs to the ? record
        ];
        assert_eq!(record_starts(&lines), vec![0, 1, 4]);
        assert_eq!(record_span(&lines, 0), (0, 1));
        assert_eq!(record_span(&lines, 2), (1, 4));
        assert_eq!(record_span(&lines, 5), (4, 6));
        assert_eq!(record_text(&lines, 2), ". Fix build @herdr\ndetail jeden\ndetail dva");
    }
}