//! SPAI markdown formatting and body status prefix updates.
use crate::slices::spai_notes::note::{SpaiNoteItem, SpaiStatus};

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
        SpaiStatus::Skutek => "+ ",
        SpaiStatus::Mood => "= ",
        SpaiStatus::Win => "* ",
        SpaiStatus::Fuckup => "% ",
        SpaiStatus::Sleep => "~ ",
        SpaiStatus::Shopping => "$ ",
        SpaiStatus::Health => "♥ ",
        SpaiStatus::Tally => "# ",
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
        for p in &[
            "/. ", "/· ", "!. ", "!/ ", "!/. ", "!x ", "!X ", "!z ", "!Z ", "!? ", "!- ", "!+ ",
            "!= ", "!* ", "!% ", "!~ ", "!$ ", "!♥ ", "!# ", ". ", "/ ", "x ", "X ", "z ", "Z ",
            "? ", "- ", "+ ", "= ", "* ", "% ", "~ ", "$ ", "♥ ", "h ", "# ",
        ] {
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

/// Builds markdown content with standard SPAI YAML frontmatter and 5D facets.
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
        || item.facets.area.is_some()
        || item.facets.effort.is_some()
        || item.facets.urgency.is_some()
        || item.facets.who.is_some()
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
        if let Some(a) = &item.facets.area {
            out.push_str(&format!("  area: {}\n", a));
        }
        if let Some(e) = &item.facets.effort {
            out.push_str(&format!("  effort: {}\n", e));
        }
        if let Some(u) = &item.facets.urgency {
            out.push_str(&format!("  urgency: {}\n", u));
        }
        if let Some(w) = &item.facets.who {
            out.push_str(&format!("  who: {}\n", w));
        }
    }

    out.push_str(&format!("spai_symbol: '{}'\n", item.symbol));
    out.push_str("---\n\n");

    out.push_str(&format!("# {}: {}\n\n", item.id, item.title));
    out.push_str(item.body.trim());
    out.push('\n');

    out
}
