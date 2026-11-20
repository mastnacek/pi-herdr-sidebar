//! SPAI markdown formatting and body status prefix updates.
use crate::slices::spai_notes::note::SpaiNoteItem;

/// Escapes a YAML double-quoted scalar (backslash first, then the quote).
fn escape_yaml_scalar(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Builds markdown content with standard SPAI YAML frontmatter and 5D facets.
///
/// Unknown frontmatter keys captured by the parser ([`SpaiNoteItem::extra_frontmatter`])
/// are re-emitted verbatim so that edits and status cycles are lossless for
/// keys this tool does not model. The `source:` default is only written when
/// the file does not already carry its own value.
pub fn format_spai_markdown(item: &SpaiNoteItem) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("type: {}\n", item.kind.as_str()));
    out.push_str(&format!("title: \"{}\"\n", escape_yaml_scalar(&item.title)));
    out.push_str(&format!("timestamp: {}\n", item.timestamp));
    out.push_str(&format!("status: {}\n", item.status.as_str()));
    if !item.extra_frontmatter.iter().any(|(k, _)| k == "source") {
        out.push_str("source: pi-spai\n");
    }

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
    for (key, value) in &item.extra_frontmatter {
        out.push_str(&format!("{}: {}\n", key, value));
    }
    out.push_str("---\n\n");

    out.push_str(&format!("# {}: {}\n\n", item.id, item.title));
    out.push_str(item.body.trim());
    out.push('\n');

    out
}
