//! SPAI markdown parsing and frontmatter extraction.
use crate::slices::spai_notes::note::{SpaiFacets, SpaiNoteItem, SpaiStatus, SpaiType};
use std::path::PathBuf;

/// Parses a YAML inline tag list (`[ai, chat]`, `ai, chat`, or a single tag)
/// into a vector, stripping quotes and empty entries.
pub fn parse_inline_tags(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(trimmed);
    inner
        .split(',')
        .map(|t| t.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Unescapes the YAML double-quoted scalar written by the formatter.
pub fn unescape_yaml_scalar(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Splits `content` into the YAML frontmatter part and the body.
///
/// The frontmatter ends at a line that is exactly `---` (a `\n----` line or a
/// `---` inside a value does **not** terminate it).
fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let (without_opening, start_len) = if let Some(rest) = content.strip_prefix("---\r\n") {
        (rest, 5)
    } else if let Some(rest) = content.strip_prefix("---\n") {
        (rest, 4)
    } else {
        return (None, content);
    };

    let mut offset = start_len;
    for line in without_opening.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        offset += line.len();
        if trimmed.trim() == "---" {
            let yaml = &content[start_len..offset - line.len()];
            let body_start = offset;
            let body = &content[body_start..];
            let body = body
                .strip_prefix("\r\n")
                .or_else(|| body.strip_prefix('\n'))
                .unwrap_or(body);
            return (Some(yaml), body);
        }
    }
    (None, content)
}

/// Header extraction: `# SPAI-XXX: Title` or `# Title`, returning the title
/// and the byte offset of the first body byte after the header line.
fn extract_header(body_part: &str) -> (String, String, usize) {
    let mut id = String::new();
    let mut title = String::new();
    let mut header_end_offset = 0;

    let mut offset = 0;
    for line in body_part.split('\n') {
        let line_without_cr = line.strip_suffix('\r').unwrap_or(line);
        let t = line_without_cr.trim();
        if !t.starts_with('#') {
            // Skip empty lines and prose until the first heading, like the
            // original line-scan did.
            offset += line.len() + 1;
            continue;
        }
        let h = t.trim_start_matches('#').trim();
        if let Some((lhs, rhs)) = h.split_once(':') {
            if lhs.trim().to_ascii_uppercase().starts_with("SPAI-") {
                id = lhs.trim().to_string();
                title = rhs.trim().to_string();
            }
        }
        if title.is_empty() {
            title = h.to_string();
        }
        header_end_offset = offset + line_without_cr.len();
        break;
    }

    (id, title, header_end_offset)
}

/// Derives the SPAI id from the filename (`2026-09-25-SPAI-002-slug.md`).
fn id_from_filename(file_name: &str) -> Option<String> {
    let idx = file_name.find("SPAI-")?;
    let rest = &file_name[idx..];
    let end = rest.find('-')?;
    let num_end = rest[end + 1..]
        .find(|c: char| !c.is_ascii_digit())
        .map(|i| end + 1 + i)
        .unwrap_or(rest.len());
    Some(rest[..num_end].to_string())
}

/// Keys this parser models; anything else in the frontmatter is preserved
/// verbatim in [`SpaiNoteItem::extra_frontmatter`].
const KNOWN_KEYS: &[&str] = &[
    "type",
    "status",
    "timestamp",
    "tags",
    "spai_symbol",
    "project",
    "project_path",
    "priority",
    "deadline",
    "area",
    "effort",
    "urgency",
    "who",
    "title",
    "facets",
];

/// Parses a SPAI markdown file into `SpaiNoteItem`.
pub fn parse_spai_markdown(content: &str, file_path: PathBuf) -> Option<SpaiNoteItem> {
    let file_name = file_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("")
        .to_string();

    let (yaml_part, body_part) = split_frontmatter(content);
    let yaml_part = yaml_part.unwrap_or("");

    let (mut id, mut title, header_end_offset) = extract_header(body_part);

    let clean_body = if header_end_offset > 0 {
        body_part[header_end_offset..].trim_start().to_string()
    } else {
        body_part.to_string()
    };

    // Fallback ID from the filename; "SPAI-???" (not a reused number) when the
    // filename carries none, so two such files never collide on one id.
    if id.is_empty() {
        id = id_from_filename(&file_name).unwrap_or_else(|| "SPAI-???".to_string());
    }

    // Parse frontmatter keys
    let mut kind = SpaiType::Todo;
    let mut status = SpaiStatus::Todo;
    let mut timestamp = String::new();
    let mut tags: Vec<String> = Vec::new();
    let mut tags_list_mode = false;
    let mut facets = SpaiFacets::default();
    let mut symbol = String::new();
    let mut extra_frontmatter: Vec<(String, String)> = Vec::new();
    let mut inside_unknown_block = false;

    for line in yaml_part.lines() {
        let l = line.trim();

        // Multi-line YAML list form: `tags:` followed by `- tag` lines
        if tags_list_mode {
            if let Some(v) = l.strip_prefix("- ") {
                let tag = v.trim().trim_matches('"').trim_matches('\'');
                if !tag.is_empty() {
                    tags.push(tag.to_string());
                }
                continue;
            }
            tags_list_mode = false;
        }

        // Indented lines belong to a nested block (e.g. `facets:` subkeys).
        if line.starts_with(' ') || line.starts_with('\t') {
            if inside_unknown_block {
                continue; // drop nested content of unknown keys; scalar keys are preserved
            }
        } else {
            inside_unknown_block = false;
        }

        if let Some(v) = l.strip_prefix("type:") {
            kind = SpaiType::from_str(v);
        } else if let Some(v) = l.strip_prefix("status:") {
            status = SpaiStatus::from_str(v);
        } else if let Some(v) = l.strip_prefix("timestamp:") {
            timestamp = v.trim().trim_matches('"').to_string();
        } else if let Some(v) = l.strip_prefix("tags:") {
            let v = v.trim();
            if v.is_empty() {
                tags_list_mode = true;
            } else {
                tags = parse_inline_tags(v);
            }
        } else if let Some(v) = l.strip_prefix("spai_symbol:") {
            symbol = v.trim().trim_matches('\'').trim_matches('"').to_string();
        } else if let Some(v) = l.strip_prefix("project:") {
            facets.project = Some(v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("project_path:") {
            facets.project_path = Some(v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("priority:") {
            facets.priority = Some(v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("deadline:") {
            facets.deadline = Some(v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("area:") {
            facets.area = Some(v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("effort:") {
            facets.effort = Some(v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("urgency:") {
            facets.urgency = Some(v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("who:") {
            facets.who = Some(v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("title:") {
            if title.is_empty() {
                title = unescape_yaml_scalar(v.trim().trim_matches('"'));
            }
        } else if let Some((key, value)) = l.split_once(':') {
            // Unknown key: preserve scalar values verbatim, remember the block
            // until the next non-indented line, and skip empty markers.
            let key = key.trim();
            let value = value.trim();
            if !key.is_empty() && !value.is_empty() {
                extra_frontmatter.push((key.to_string(), value.to_string()));
            }
            // `facets:` is a known nested block; only truly unknown keys open
            // an indented block whose lines are skipped.
            inside_unknown_block = key != "facets";
        }
    }

    if symbol.is_empty() {
        symbol = status.symbol().to_string();
    }

    if title.is_empty() {
        title = file_name.clone();
    }

    Some(SpaiNoteItem {
        id,
        title,
        kind,
        status,
        symbol,
        timestamp,
        tags,
        facets,
        body: clean_body,
        file_path,
        file_name,
        extra_frontmatter,
    })
}
