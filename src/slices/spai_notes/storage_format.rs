//! Parsing and formatting for SPAI notes markdown and YAML frontmatter.
use std::path::PathBuf;
use super::note::{SpaiFacets, SpaiNoteItem, SpaiStatus, SpaiType};

/// Creates a safe URL/file slug from title.
pub fn slugify(text: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;

    for c in text.chars() {
        if c.is_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if (c == ' ' || c == '_' || c == '-') && !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
    }

    let trimmed = slug.trim_end_matches('-');
    if trimmed.len() > 50 {
        trimmed[..50].trim_end_matches('-').to_string()
    } else {
        trimmed.to_string()
    }
}

/// Updates the leading SPAI prefix on the first non-empty line of the markdown body (compatible with pi-spai).
pub fn update_body_status_prefix(body: &str, status: SpaiStatus) -> String {
    let new_prefix = match status {
        SpaiStatus::Working => "/ ",
        SpaiStatus::Waiting => "/. ",
        SpaiStatus::Done => "x ",
        SpaiStatus::Cancelled => "z ",
        SpaiStatus::Idea => "? ",
        SpaiStatus::Note | SpaiStatus::Inbox => "- ",
        SpaiStatus::Todo => ". ",
    };

    let mut lines: Vec<String> = body.lines().map(|s| s.to_string()).collect();
    let mut updated = false;

    for line in &mut lines {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }

        let indent = &line[..line.len() - trimmed.len()];
        let mut matched_len = 0;
        for p in &["/. ", "/· ", "!- ", ". ", "/ ", "x ", "X ", "z ", "Z ", "? ", "- "] {
            if trimmed.starts_with(p) {
                matched_len = p.len();
                break;
            }
        }

        if matched_len > 0 {
            let rest = &trimmed[matched_len..];
            *line = format!("{}{}{}", indent, new_prefix, rest);
        } else {
            *line = format!("{}{}{}", indent, new_prefix, trimmed);
        }
        updated = true;
        break;
    }

    if !updated {
        return format!("{}{}\n", new_prefix, body.trim());
    }

    lines.join("\n")
}

/// Builds markdown content with standard SPAI YAML frontmatter.
pub fn format_spai_markdown(item: &SpaiNoteItem) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("type: {}\n", item.kind.as_str()));
    out.push_str(&format!("title: \"{}\"\n", item.title.replace('"', "\\\"")));
    out.push_str(&format!("timestamp: {}\n", item.timestamp));
    out.push_str(&format!("status: {}\n", item.status.as_str()));
    out.push_str("source: pi-spai\n");

    if !item.tags.is_empty() {
        out.push_str(&format!("tags: [{}]\n", item.tags.join(", ")));
    }

    if item.facets.priority.is_some()
        || item.facets.deadline.is_some()
        || item.facets.project.is_some()
        || item.facets.project_path.is_some()
    {
        out.push_str("facets:\n");
        if let Some(p) = &item.facets.priority {
            out.push_str(&format!("  priority: {}\n", p));
        }
        if let Some(d) = &item.facets.deadline {
            out.push_str(&format!("  deadline: {}\n", d));
        }
        if let Some(proj) = &item.facets.project {
            out.push_str(&format!("  project: {}\n", proj));
        }
        if let Some(pp) = &item.facets.project_path {
            out.push_str(&format!("  project_path: {}\n", pp));
        }
    }

    out.push_str(&format!("spai_symbol: '{}'\n", item.symbol));
    out.push_str("---\n\n");

    out.push_str(&format!("# {}: {}\n\n", item.id, item.title));
    out.push_str(item.body.trim());
    out.push('\n');

    out
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

    // Parse simple frontmatter keys
    let mut kind = SpaiType::Todo;
    let mut status = SpaiStatus::Todo;
    let mut timestamp = String::new();
    let tags = Vec::new();
    let mut facets = SpaiFacets {
        project: None,
        project_path: None,
        priority: None,
        deadline: None,
    };
    let mut symbol = String::new();

    for line in yaml_part.lines() {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("type:") {
            kind = SpaiType::from_str(v);
        } else if let Some(v) = l.strip_prefix("status:") {
            status = SpaiStatus::from_str(v);
        } else if let Some(v) = l.strip_prefix("timestamp:") {
            timestamp = v.trim().trim_matches('"').to_string();
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
