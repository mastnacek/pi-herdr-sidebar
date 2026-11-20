//! Shared record writer: turns one marked SPAI text record into a physical
//! file (plan §2, phase 0). Used by the creation dialog and the Scratchpad.
//!
//! Guarantees carried over from the review fixes: the id comes from the disk
//! (`max + 1`), the file is created with `create_new` (retry on collision, so
//! `pi-spai` racing for the same id can never overwrite a file), the directory
//! is created when missing, and the caller gets back everything needed for a
//! `✓ → project SPAI-014` label.
use super::discovery::SpaiProjectSummary;
use super::input_highlighter::detect_spai_input;
use super::note_io::write_atomic;
use super::spai_prefixes::strip_leading_prefix;
use super::storage_format::{format_spai_markdown, parse_spai_markdown};

/// A Czech inline stamp `[10.2.2026 14:22:37]` (seconds optional), read back
/// into `YYYY-MM-DD HH:MM:SS`. Returns it with the text after the token.
fn take_stamp_token(s: &str) -> (String, Option<String>) {
    let Some(rest) = s.strip_prefix('[') else {
        return (s.to_string(), None);
    };
    let Some(close) = rest.find(']') else {
        return (s.to_string(), None);
    };
    let body = &rest[..close];
    let after = rest[close + 1..].trim_start();
    let Some((date, time)) = body.split_once(' ') else {
        return (s.to_string(), None);
    };
    let mut d = date.split('.');
    let (day, month, year) = match (d.next(), d.next(), d.next(), d.next()) {
        (Some(dd), Some(mm), Some(yy), None)
            if !dd.is_empty() && !mm.is_empty() && !yy.is_empty() =>
        {
            (dd, mm, yy)
        }
        _ => return (s.to_string(), None),
    };
    let mut t = time.split(':');
    let (h, mi, s2) = match (t.next(), t.next(), t.next(), t.next()) {
        (Some(h), Some(mi), None, None) => (h, mi, "0"),
        (Some(h), Some(mi), Some(s), None) => (h, mi, s),
        _ => return (s.to_string(), None),
    };
    let ok = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit());
    if ![day, month, year, h, mi, s2].iter().all(|p| ok(p)) {
        return (s.to_string(), None);
    }
    let iso = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year.parse::<u32>().unwrap_or(0),
        month.parse::<u32>().unwrap_or(0),
        day.parse::<u32>().unwrap_or(0),
        h.parse::<u32>().unwrap_or(0),
        mi.parse::<u32>().unwrap_or(0),
        s2.parse::<u32>().unwrap_or(0),
    );
    (after.to_string(), Some(iso))
}

/// Rewrites an existing note file with new record `text` (bidirectional
/// editing): the id, file path, tags, facets and extra frontmatter stay; the
/// title, type/status, body and (when the line carries an inline stamp) the
/// timestamp come from the edited text. The file name is NOT renamed — the
/// id stays the anchor. A changed Czech title (`title_cs`) never touches the
/// English `title` or the SPAI line: it adds `retitle_en` to `pending`.
/// Returns true when `retitle_en` was added (`spai reindex` should run).
pub fn update_record(path: &std::path::Path, text: &str) -> Result<bool, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("Čtení selhalo: {}", e))?;
    let mut item = parse_spai_markdown(&content, path.to_path_buf())
        .ok_or_else(|| "Soubor nelze načíst jako SPAI poznámku".to_string())?;

    let first_line = text.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return Err("Prázdný záznam".to_string());
    }
    let detected = detect_spai_input(first_line);
    let clean = strip_leading_prefix(first_line)
        .unwrap_or(first_line)
        .trim();
    let (clean, typed_stamp) = take_stamp_token(clean);
    let before = format_spai_markdown(&item);
    let old_title = item.title.clone();
    // The scratch line shows the Czech title: unchanged text keeps both titles.
    let had_cs = item.title_cs().is_some();
    let mut retitled = false;
    if !clean.is_empty() && clean != item.display_title() {
        retitled = item.retitle_cs(&clean);
        if !retitled {
            item.title = clean.to_string();
        }
    }
    item.on_title_changed(&old_title);
    item.kind = detected.kind;
    item.status = detected.status;
    item.symbol = detected.status.symbol().to_string();
    let mut new_first = text.trim().lines().next().unwrap_or("").to_string();
    if had_cs && !clean.is_empty() {
        // The first line shows the Czech title; the file keeps the English one.
        new_first = new_first.replacen(clean.as_str(), &item.title, 1);
    }
    let continuation: Vec<&str> = text.trim().lines().skip(1).collect();
    let typed_prefix = &first_line[..first_line.len() - strip_leading_prefix(first_line).unwrap_or(first_line).len()];
    item.body = merge_body(
        &item.body,
        (typed_prefix, &new_first),
        (&old_title, &item.title),
        &continuation,
    );
    if let Some(ts) = typed_stamp {
        item.timestamp = ts;
    }

    let after = format_spai_markdown(&item);
    if after != before {
        write_atomic(path, &after)?;
    }
    Ok(retitled)
}

/// The old `body` with only the mark and the title of its SPAI line changed
/// (`!priority`, `@project`, `:tags:` stay); every other line stays. `typed` is
/// the mark and the whole edited line (used when the body has no SPAI line),
/// `titles` the old and the new title. Typed continuation lines the body does
/// not have yet are appended; none is ever removed.
fn merge_body(
    body: &str,
    typed: (&str, &str),
    titles: (&str, &str),
    continuation: &[&str],
) -> String {
    let (mark, first) = typed;
    let mut lines: Vec<String> = body.lines().map(String::from).collect();
    let spai_line = lines.iter().position(|l| !l.trim().is_empty());
    match spai_line.and_then(|i| strip_leading_prefix(&lines[i]).map(|t| (i, t.to_string()))) {
        Some((i, tail)) => {
            let tail = match tail.strip_prefix(titles.0) {
                Some(rest) => format!("{}{rest}", titles.1),
                None => tail,
            };
            lines[i] = format!("{mark}{tail}");
        }
        None => {
            lines.insert(0, String::new());
            lines.insert(0, first.to_string());
        }
    }
    let mut added = Vec::new();
    for line in continuation.iter().map(|l| l.trim_end()).filter(|l| !l.trim().is_empty()) {
        if !lines.iter().any(|l| l.trim_end() == line) {
            added.push(line.to_string());
        }
    }
    if !added.is_empty() {
        lines.push(String::new());
        lines.extend(added);
    }
    lines.join("\n").trim().to_string()
}

/// `text` without the inline stamp the Scratchpad puts after the mark prefix
/// (`. [10.2.2026 14:22:37] Název` → `. Název`): `spai` stamps records itself.
pub fn without_stamp(text: &str) -> String {
    let (first, rest) = match text.split_once('\n') {
        Some((f, r)) => (f, Some(r)),
        None => (text, None),
    };
    let tail = strip_leading_prefix(first).unwrap_or(first);
    let prefix = &first[..first.len() - tail.len()];
    let (clean, stamp) = take_stamp_token(tail.trim());
    let mut out = if stamp.is_some() {
        format!("{}{}", prefix, clean.trim())
    } else {
        first.to_string()
    };
    if let Some(r) = rest {
        out.push('\n');
        out.push_str(r);
    }
    out
}

/// Finds the project cited with the first `@mention` in `text` (name or path,
/// case-insensitive). Returns `None` when nothing matches.
pub fn route_mention<'a>(
    text: &str,
    projects: &'a [SpaiProjectSummary],
) -> Option<&'a SpaiProjectSummary> {
    for token in text.split_whitespace() {
        let Some(mention) = token.strip_prefix('@') else {
            continue;
        };
        let mention = mention.trim_matches('"').to_lowercase();
        if mention.is_empty() {
            continue;
        }
        for p in projects {
            let name = p.name.to_lowercase();
            let path = p.path.to_string_lossy().to_lowercase();
            if name == mention || name.contains(&mention) || path.contains(&mention) {
                return Some(p);
            }
        }
    }
    None
}

/// Fallback routing: the currently active project, if it is among `projects`.
pub fn current_project<'a>(
    current_path: Option<&std::path::Path>,
    projects: &'a [SpaiProjectSummary],
) -> Option<&'a SpaiProjectSummary> {
    let cp = current_path?;
    projects.iter().find(|p| p.path == cp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn routes_the_first_project_mention() {
        let a = SpaiProjectSummary::new(
            "herdr".to_string(),
            PathBuf::from("/tmp/herdr"),
            PathBuf::from("/tmp/herdr/docs/spai"),
        );
        let b = SpaiProjectSummary::new(
            "pi-spai".to_string(),
            PathBuf::from("/tmp/pi-spai"),
            PathBuf::from("/tmp/pi-spai/docs/spai"),
        );
        let projects = vec![a, b];

        let routed = route_mention(". Fix build @pi-spai", &projects).unwrap();
        assert_eq!(routed.name, "pi-spai");
        let routed = route_mention(". Fix @\"herdr\" build", &projects).unwrap();
        assert_eq!(routed.name, "herdr");
        assert!(route_mention(". Nic @neexistuje", &projects).is_none());
        assert!(route_mention(". Nic", &projects).is_none());
    }

    #[test]
    fn the_inline_stamp_is_removed_before_add() {
        assert_eq!(
            without_stamp(". [10.2.2026 14:22:37] Opravit build @proj\ndetail"),
            ". Opravit build @proj\ndetail"
        );
        assert_eq!(without_stamp(". Bez razítka"), ". Bez razítka");
        assert_eq!(
            without_stamp("- [ai, chat] Značky nejsou čas"),
            "- [ai, chat] Značky nejsou čas"
        );
    }
}
