//! Section-body text handling: wrapper stripping, previews and the
//! `<project_instructions>` parser used by the transcript replay.

use super::{PromptFileRef, SourceState};

/// Drop the `<name>…</name>` wrapper pi adds around every section except
/// `preamble`, so sizes and previews describe the text itself.
pub(super) fn strip_wrapper(name: &str, text: &str) -> String {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let mut body = text.trim();
    if let Some(rest) = body.strip_prefix(&open) {
        body = rest.trim_start_matches('\n');
    }
    if let Some(rest) = body.strip_suffix(&close) {
        body = rest.trim_end_matches(['\n', ' ']);
    }
    body.to_string()
}

/// First meaningful line of a section body, skipping the `<name>…</name>`
/// wrapper pi adds.
pub(super) fn preview(text: &str, name: &str) -> String {
    let open = format!("<{name}>");
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| {
            !l.is_empty() && *l != open && *l != "Project-specific instructions and guidelines:"
        })
        .unwrap_or("");
    let mut out: String = line.chars().take(64).collect();
    if line.chars().count() > 64 {
        out.push('…');
    }
    out
}

/// Extract `<project_instructions path="…">…</project_instructions>` entries
/// from the `project_context` section.
pub(super) fn parse_project_instructions(text: &str) -> Vec<PromptFileRef> {
    const OPEN: &str = "<project_instructions path=\"";
    const CLOSE: &str = "</project_instructions>";

    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        let after = &rest[start + OPEN.len()..];
        let Some(quote) = after.find('"') else {
            break;
        };
        let path = after[..quote].to_string();
        let Some(gt) = after[quote..].find('>') else {
            break;
        };
        let body = &after[quote + gt + 1..];
        let (content, next) = match body.find(CLOSE) {
            Some(end) => (&body[..end], &body[end + CLOSE.len()..]),
            None => (body, ""),
        };
        out.push(PromptFileRef {
            path,
            chars: content.trim().chars().count(),
            state: SourceState::Ok,
        });
        rest = next;
    }
    out
}
