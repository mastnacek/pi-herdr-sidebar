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

/// Parses a SPAI markdown file into `SpaiNoteItem`.
pub fn parse_spai_markdown(content: &str, file_path: PathBuf) -> Option<SpaiNoteItem> {
    let file_name = file_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("")
        .to_string();

    let mut yaml_part = "";
    let mut body_part = content;

    if content.starts_with("---\n") || content.starts_with("---\r\n") {
        let start = if content.starts_with("---\r\n") { 5 } else { 4 };
        if let Some(end) = content[start..].find("\n---") {
            yaml_part = &content[start..start + end];
            body_part = content[start + end + 4..].trim_start();
            if body_part.starts_with("\r\n") {
                body_part = &body_part[2..];
            } else if body_part.starts_with('\n') {
                body_part = &body_part[1..];
            }
        }
    }

    // Header extraction: # SPAI-XXX: Title OR # Title
    let mut id = String::new();
    let mut title = String::new();
    let mut header_end_offset = 0;

    for line in body_part.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            let h = t.trim_start_matches('#').trim();
            if let Some((lhs, rhs)) = h.split_once(':') {
                let lhs = lhs.trim();
                if lhs.to_ascii_uppercase().starts_with("SPAI-") {
                    id = lhs.to_string();
                    title = rhs.trim().to_string();
                }
            }
            if title.is_empty() {
                title = h.to_string();
            }

            // Find where this header line ends in body_part to strip it from body
            if let Some(pos) = body_part.find(line) {
                header_end_offset = pos + line.len();
            }
            break;
        }
    }

    let clean_body = if header_end_offset > 0 {
        body_part[header_end_offset..].trim_start().to_string()
    } else {
        body_part.to_string()
    };

    // Try fallback ID from filename e.g. 2026-09-25-SPAI-002-...
    if id.is_empty() {
        if let Some(idx) = file_name.find("SPAI-") {
            let rest = &file_name[idx..];
            let end = rest.find('-').unwrap_or(rest.len());
            let num_end = rest[end + 1..]
                .find(|c: char| !c.is_ascii_digit())
                .map(|i| end + 1 + i)
                .unwrap_or(rest.len());
            id = rest[..num_end].to_string();
        } else {
            id = "SPAI-001".to_string();
        }
    }

    // Parse frontmatter keys
    let mut kind = SpaiType::Todo;
    let mut status = SpaiStatus::Todo;
    let mut timestamp = String::new();
    let mut tags: Vec<String> = Vec::new();
    let mut tags_list_mode = false;
    let mut facets = SpaiFacets::default();
    let mut symbol = String::new();

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
                title = v.trim().trim_matches('"').to_string();
            }
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
    })
}
