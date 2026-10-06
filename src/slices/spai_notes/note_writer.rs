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
use super::note::{SpaiFacets, SpaiNoteItem};
use super::note_io::{create_note_file, next_spai_number};
use super::spai_prefixes::strip_leading_prefix;
use super::storage_format::format_spai_markdown;
use super::time_utils::current_timestamp_and_date;
use std::path::PathBuf;

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
        (Some(dd), Some(mm), Some(yy), None) if !dd.is_empty() && !mm.is_empty() && !yy.is_empty() => {
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

/// The `YYYY-MM-DD` part of an ISO timestamp, for the file-name date.
fn today_part(iso: &str) -> Option<String> {
    iso.get(..10).map(String::from)
}

pub struct WrittenNote {
    pub id: String,
    pub path: PathBuf,
    pub project: String,
    pub item: SpaiNoteItem,
}

/// Writes `text` (a record: first line marked, continuation lines optional)
/// as a new note file in `proj`'s docs/spai directory.
pub fn write_record(proj: &SpaiProjectSummary, text: &str) -> Result<WrittenNote, String> {
    let first_line = text.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return Err("Prázdný záznam".to_string());
    }

    let detected = detect_spai_input(first_line);
    let clean_title = strip_leading_prefix(first_line).unwrap_or(first_line).trim();

    // A Scratchpad line carries its stamp inline (`[10.2.2026 14:22:37]`,
    // inserted right after the prefix): use it as the note's timestamp —
    // the date of *typing* — and keep it out of the title and body.
    let (clean_title, typed_stamp) = take_stamp_token(clean_title);
    let clean_title: &str = clean_title.trim();
    let title = if clean_title.is_empty() {
        first_line.to_string()
    } else {
        clean_title.to_string()
    };

    std::fs::create_dir_all(&proj.spai_dir)
        .map_err(|e| format!("Složku docs/spai nelze vytvořit: {}", e))?;

    let (today, saved_stamp) = current_timestamp_and_date();
    let timestamp = typed_stamp.unwrap_or(saved_stamp);
    let today = today_part(&timestamp).unwrap_or(today);
    let slug = slugify(&title);
    // The body keeps the record exactly as typed (stamp included) — the
    // frontmatter timestamp is the parsed one; nothing the user wrote is lost.
    let body = text.trim().to_string();

    // create_new refuses on collision; bump the id and retry.
    for _ in 0..100 {
        let next_num = next_spai_number(&proj.spai_dir, &proj.items);
        let id = format!("SPAI-{:03}", next_num);
        let file_path = proj.spai_dir.join(format!("{}-{}-{}.md", today, id, slug));
        let file_name = file_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("")
            .to_string();

        let item = SpaiNoteItem {
            id: id.clone(),
            title: title.clone(),
            kind: detected.kind,
            status: detected.status,
            symbol: detected.status.symbol().to_string(),
            timestamp: timestamp.clone(),
            tags: Vec::new(),
            facets: SpaiFacets {
                project: Some(proj.name.clone()),
                project_path: Some(proj.path.to_string_lossy().to_string()),
                ..Default::default()
            },
            body: body.clone(),
            file_path: file_path.clone(),
            file_name,
            extra_frontmatter: Vec::new(),
        };

        match create_note_file(&file_path, &format_spai_markdown(&item)) {
            Ok(()) => {
                return Ok(WrittenNote {
                    id,
                    path: file_path,
                    project: proj.name.clone(),
                    item,
                })
            }
            Err(_) => continue, // collision: try the next id
        }
    }
    Err("Nelze najít volné SPAI ID (100 pokusů)".to_string())
}

fn slugify(text: &str) -> String {
    super::storage_format::slugify(text)
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
    use std::fs;
    use std::path::PathBuf;

    fn temp_project(tag: &str) -> SpaiProjectSummary {
        let dir = std::env::temp_dir().join(format!("spai_writer_{}_{}", tag, std::process::id()));
        fs::create_dir_all(dir.join("docs").join("spai")).unwrap();
        SpaiProjectSummary::new("proj".to_string(), dir.clone(), dir.join("docs").join("spai"))
    }

    #[test]
    fn writes_a_marked_record_with_disk_backed_id() {
        let proj = temp_project("basic");
        let note = write_record(&proj, ". Opravit build @projekt").unwrap();
        assert!(note.path.exists(), "file created: {:?}", note.path);
        assert_eq!(note.id, "SPAI-001");
        assert_eq!(note.project, "proj");
        let content = fs::read_to_string(&note.path).unwrap();
        assert!(content.contains("title: \"Opravit build @projekt\""));
        assert!(content.contains("# SPAI-001: Opravit build @projekt"));

        let second = write_record(&proj, "? Nový nápad").unwrap();
        assert_eq!(second.id, "SPAI-002", "id continues from the disk max");
        fs::remove_dir_all(&proj.path).ok();
    }

    #[test]
    fn continuation_lines_become_the_body() {
        let proj = temp_project("body");
        let text = ". Úkol\n\ndetail první\ndetail druhý";
        let note = write_record(&proj, text).unwrap();
        let content = fs::read_to_string(&note.path).unwrap();
        assert!(content.contains("detail první"), "body: {content}");
        assert!(content.contains("detail druhý"), "body: {content}");
        fs::remove_dir_all(&proj.path).ok();
    }

    #[test]
    fn never_overwrites_an_existing_file() {
        let proj = temp_project("collision");
        let first = write_record(&proj, ". První").unwrap();
        // The id counter skips used ids, so a second write never collides.
        let second = write_record(&proj, ". První").unwrap();
        assert_ne!(first.path, second.path);
        assert_eq!(fs::read_to_string(&first.path).unwrap().contains("# SPAI-001"), true);
        assert_eq!(fs::read_to_string(&second.path).unwrap().contains("# SPAI-002"), true);
        fs::remove_dir_all(&proj.path).ok();
    }

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
    fn an_inline_stamp_becomes_the_note_timestamp_and_leaves_the_title() {
        let proj = temp_project("stamp");
        let note = write_record(&proj, ". [10.2.2026 14:22:37] Opravit build @proj").unwrap();
        let content = fs::read_to_string(&note.path).unwrap();
        // Title is clean; the stamp moved into the frontmatter timestamp.
        assert!(content.contains("# SPAI-001: Opravit build"), "title: {content}");
        assert!(content.contains("timestamp: 2026-02-10 14:22:37"), "ts: {content}");
        // The file name carries the *typed* date, not the save date.
        assert!(
            note.path.file_name().unwrap().to_string_lossy().starts_with("2026-02-10-"),
            "name: {:?}",
            note.path
        );
        // The body keeps exactly what was typed.
        assert!(content.contains(". [10.2.2026 14:22:37] Opravit build @proj"), "body: {content}");
        fs::remove_dir_all(&proj.path).ok();
    }

    #[test]
    fn a_stamp_with_minutes_only_is_accepted_and_a_yaml_list_is_not() {
        let proj = temp_project("stamp2");
        let note = write_record(&proj, ". [10.2.2026 14:22] Krátký čas").unwrap();
        let content = fs::read_to_string(&note.path).unwrap();
        assert!(content.contains("timestamp: 2026-02-10 14:22:00"), "ts: {content}");

        let note2 = write_record(&proj, "- [ai, chat] Značky nejsou čas").unwrap();
        let content2 = fs::read_to_string(&note2.path).unwrap();
        assert!(
            content2.contains(&format!("title: \"[ai, chat] Značky nejsou čas\"")),
            "a non-stamp bracket stays in the title: {content2}"
        );
        fs::remove_dir_all(&proj.path).ok();
    }
}
