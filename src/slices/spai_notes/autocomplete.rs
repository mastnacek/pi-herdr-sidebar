//! Autocomplete suggestions for @project insertion (compatible with pi-spai).
use super::discovery::SpaiProjectSummary;

#[derive(Debug, Clone)]
pub struct ProjectSuggestion {
    pub name: String,
    pub path: String,
    pub insert_text: String,
}

/// Detects if input ends with `@...` trigger and returns query if active.
pub fn extract_at_query(input: &str) -> Option<&str> {
    if let Some(idx) = input.rfind('@') {
        let after_at = &input[idx + 1..];
        // Only active if no whitespace inside token
        if !after_at.contains(|c: char| c.is_whitespace()) {
            return Some(after_at);
        }
    }
    None
}

/// Filters available projects matching the `@query`.
pub fn get_project_suggestions(
    projects: &[SpaiProjectSummary],
    query: &str,
) -> Vec<ProjectSuggestion> {
    let lower_q = query.to_lowercase();
    let mut matches = Vec::new();

    for p in projects {
        let name_lower = p.name.to_lowercase();
        let path_lower = p.path.to_string_lossy().to_lowercase();

        if query.is_empty() || name_lower.contains(&lower_q) || path_lower.contains(&lower_q) {
            let insert_name = if p.name.contains(' ') {
                format!("@\"{}\"", p.name)
            } else {
                format!("@{}", p.name)
            };

            matches.push(ProjectSuggestion {
                name: p.name.clone(),
                path: p.path.to_string_lossy().to_string(),
                insert_text: insert_name,
            });
        }
    }

    matches.truncate(8);
    matches
}

/// Applies completion of selected suggestion into input string.
pub fn apply_at_completion(input: &mut String, suggestion: &ProjectSuggestion) {
    if let Some(idx) = input.rfind('@') {
        input.truncate(idx);
        input.push_str(&suggestion.insert_text);
        input.push(' ');
    }
}
