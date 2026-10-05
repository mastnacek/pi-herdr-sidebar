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
    let title = if clean_title.is_empty() {
        first_line.to_string()
    } else {
        clean_title.to_string()
    };

    std::fs::create_dir_all(&proj.spai_dir)
        .map_err(|e| format!("Složku docs/spai nelze vytvořit: {}", e))?;

    let (today, timestamp) = current_timestamp_and_date();
    let slug = slugify(&title);
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
}