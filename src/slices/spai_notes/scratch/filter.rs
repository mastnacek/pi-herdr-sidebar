//! Scratchpad filter: token parser, fuzzy scoring, status filters (plan §4).
//!
//! Tokens in one line compose with AND: `/build @herdr :ui: !high` — fuzzy
//! text, project, tag, priority. Fuzzy is a subsequence score with a word-start
//! bonus over Czech-normalised text (pure, crate-free).
use super::line_model::{LineOrigin, ScratchLine};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusFilter {
    #[default]
    All,
    Open,
    Done,
}

impl StatusFilter {
    pub fn next(self) -> Self {
        match self {
            StatusFilter::Open => StatusFilter::All,
            StatusFilter::All => StatusFilter::Done,
            StatusFilter::Done => StatusFilter::Open,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            StatusFilter::Open => "otevřené",
            StatusFilter::All => "vše",
            StatusFilter::Done => "hotové",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterQuery {
    /// Fuzzy text token (everything that is not @/ :/ !).
    pub text: Option<String>,
    pub project: Option<String>,
    pub tag: Option<String>,
    pub priority: Option<String>,
}

impl FilterQuery {
    /// Parses `build @herdr :ui: !high` into tokens (a single leading `/` is
    /// the mode indicator and is stripped, so the displayed line round-trips).
    pub fn parse(input: &str) -> Self {
        let input = input.strip_prefix('/').unwrap_or(input);
        let mut q = FilterQuery::default();
        let mut text_parts: Vec<&str> = Vec::new();

        for token in input.split_whitespace() {
            if let Some(p) = token.strip_prefix('@') {
                if !p.is_empty() {
                    q.project = Some(p.trim_matches('"').to_lowercase());
                    continue;
                }
            } else if let Some(t) = token.strip_prefix(':') {
                let t = t.strip_suffix(':').unwrap_or(t);
                if !t.is_empty() {
                    q.tag = Some(t.to_lowercase());
                    continue;
                }
            } else if let Some(pr) = token.strip_prefix('!') {
                q.priority = Some(pr.to_lowercase());
                continue;
            }
            text_parts.push(token);
        }

        let text = text_parts.join(" ");
        if !text.is_empty() {
            q.text = Some(text);
        }
        q
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.project.is_none()
            && self.tag.is_none()
            && self.priority.is_none()
    }
}

/// Subsequence fuzzy score in 0.0..=1.0 (`None` when the needle is not a
/// subsequence). Word-start and consecutive matches score higher; both sides
/// are Czech-normalised (diacritics folded) via the similarity module.
pub fn fuzzy_score(needle: &str, haystack: &str) -> Option<f64> {
    let n = super::super::similarity::normalize_czech(needle.trim());
    let h = super::super::similarity::normalize_czech(haystack);
    if n.is_empty() {
        return Some(0.0);
    }

    let h_chars: Vec<char> = h.chars().collect();
    let mut needle_chars = n.chars().peekable();
    let mut score = 0.0f64;
    let mut last_idx: Option<usize> = None;

    for (i, &c) in h_chars.iter().enumerate() {
        let Some(&want) = needle_chars.peek() else { break };
        if c == want {
            needle_chars.next();
            score += 1.0;
            let word_start = i == 0 || !h_chars[i - 1].is_alphanumeric();
            if word_start {
                score += 0.5;
            }
            if last_idx == Some(i.checked_sub(1).unwrap_or(usize::MAX)) {
                score += 0.25;
            }
            last_idx = Some(i);
        }
    }

    if needle_chars.peek().is_some() {
        return None; // not a subsequence
    }

    let max = n.chars().count() as f64 * 1.75; // 1.0 + 0.5 word-start + 0.25 chain
    Some((score / max).clamp(0.0, 1.0))
}

/// The record's full text: marked first line + continuation lines.
pub fn record_fuzzy_text(lines: &[ScratchLine], idx: usize) -> String {
    let (start, end) = super::line_model::record_span(lines, idx);
    lines[start..end]
        .iter()
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Is this record "done"? A file-backed record whose first line carries the
/// `x` mark; unsaved records are always open.
fn record_is_done(lines: &[ScratchLine], idx: usize) -> bool {
    let (start, _) = super::line_model::record_span(lines, idx);
    let first = &lines[start];
    match &first.origin {
        LineOrigin::New => false,
        _ => super::line_model::is_marked(&first.text)
            && first.text.trim_start().to_lowercase().starts_with('x'),
    }
}

/// Does the record at `idx` match the query (AND over all tokens)?
pub fn matches(
    lines: &[ScratchLine],
    idx: usize,
    query: &FilterQuery,
    status: StatusFilter,
    semantic_allowed: Option<&[usize]>,
) -> bool {
    // Status filter first (cheap).
    match status {
        StatusFilter::All => {}
        StatusFilter::Open => {
            if record_is_done(lines, idx) {
                return false;
            }
        }
        StatusFilter::Done => {
            if !record_is_done(lines, idx) {
                return false;
            }
        }
    }

    let (start, _) = super::line_model::record_span(lines, idx);
    let first = lines[start].clone();
    let full = record_fuzzy_text(lines, idx);
    let full_lower = full.to_lowercase();

    // Project token: the file-backed project, or an @mention in the text.
    if let Some(want) = &query.project {
        let hit = first
            .origin
            .project()
            .map(|p| p.to_lowercase().contains(want))
            .unwrap_or(false)
            || full_lower.contains(&format!("@{want}"));
        if !hit {
            return false;
        }
    }

    // Tag token.
    if let Some(want) = &query.tag {
        if !full_lower.contains(&format!(":{want}:")) {
            return false;
        }
    }

    // Priority token (bare `!` matches any priority).
    if let Some(want) = &query.priority {
        if want.is_empty() {
            if !full_lower.contains('!') {
                return false;
            }
        } else if !full_lower.contains(&format!("!{want}")) {
            return false;
        }
    }

    // Fuzzy text token: best of the first line and the whole record.
    if let Some(needle) = &query.text {
        let best = fuzzy_score(needle, &first.text)
            .into_iter()
            .chain(fuzzy_score(needle, &full))
            .reduce(f64::max);
        if best.is_none() {
            return false;
        }
    }

    // Semantic filter (phase 4): the allowed set, computed on Enter.
    if let Some(allowed) = semantic_allowed {
        if !allowed.contains(&idx) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests;
