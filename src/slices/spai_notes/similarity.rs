//! Vector and text similarity algorithms for semantic deduplication.
//!
//! Includes cosine similarity for OpenRouter embeddings and stem-aware
//! Jaccard similarity for live Czech text matching (ported from mozek_rust & spai_log).
use super::note::SpaiNoteItem;

/// Kosinová podobnost dvou vektorů. Vrátí 0.0 při nulové normě nebo prázdném vektoru.
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    (dot / (norm_a.sqrt() * norm_b.sqrt())).clamp(0.0, 1.0)
}

/// Odstraní českou diakritiku a převede na malá písmena pro robustní porovnání.
pub fn normalize_czech(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'Á' => 'a',
            'č' | 'Č' => 'c',
            'ď' | 'Ď' => 'd',
            'é' | 'ě' | 'É' | 'Ě' => 'e',
            'í' | 'Í' => 'i',
            'ň' | 'Ň' => 'n',
            'ó' | 'Ó' => 'o',
            'ř' | 'Ř' => 'r',
            'š' | 'Š' => 's',
            'ť' | 'Ť' => 't',
            'ú' | 'ů' | 'Ú' | 'Ů' => 'u',
            'ý' | 'Ý' => 'y',
            'ž' | 'Ž' => 'z',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

/// Stem-aware Jaccard token similarity for Czech language (from spai_log & mozek_rust).
pub fn token_jaccard(a: &str, b: &str) -> f64 {
    let norm_a = normalize_czech(a);
    let norm_b = normalize_czech(b);

    let tokens_a: Vec<&str> = norm_a
        .split_whitespace()
        .filter(|t| t.len() >= 3)
        .collect();
    let tokens_b: Vec<&str> = norm_b
        .split_whitespace()
        .filter(|t| t.len() >= 3)
        .collect();

    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }

    let mut matches = 0;
    for ta in &tokens_a {
        if tokens_b.iter().any(|tb| is_stem_match(ta, tb)) {
            matches += 1;
        }
    }

    let union_size = (tokens_a.len() + tokens_b.len()).saturating_sub(matches);
    if union_size == 0 {
        0.0
    } else {
        matches as f64 / union_size as f64
    }
}

fn is_stem_match(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    // Prefix stem match for words >= 4 chars (handles Czech noun/verb declensions)
    if a.len() >= 4 && b.len() >= 4 {
        let prefix_len = a.len().min(b.len()).min(5);
        if a[..prefix_len] == b[..prefix_len] {
            return true;
        }
    }
    false
}

/// Rychlá textová podobnost (kombinace přesné shody, token Jaccardu a podřetězců).
pub fn text_similarity(query: &str, target: &str) -> f64 {
    let q = normalize_czech(query.trim());
    let t = normalize_czech(target.trim());

    if q.is_empty() || t.is_empty() {
        return 0.0;
    }
    if q == t {
        return 1.0;
    }
    if t.contains(&q) || q.contains(&t) {
        let ratio = q.len().min(t.len()) as f64 / q.len().max(t.len()) as f64;
        return (0.70 + 0.30 * ratio).clamp(0.0, 1.0);
    }

    token_jaccard(&q, &t)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimilarNoteMatch {
    pub id: String,
    pub title: String,
    pub symbol: String,
    pub similarity: f64,
}

/// Najde existující záznamy podobné rozepsanému dotazu pro živý dedup v sidebaru.
pub fn find_similar_notes(
    query: &str,
    items: &[SpaiNoteItem],
    threshold: f64,
    max_results: usize,
) -> Vec<SimilarNoteMatch> {
    let clean_query = query.trim();
    if clean_query.len() < 3 {
        return Vec::new();
    }

    let mut matches = Vec::new();
    for item in items {
        let sim = text_similarity(clean_query, &item.title);
        if sim >= threshold {
            matches.push(SimilarNoteMatch {
                id: item.id.clone(),
                title: item.title.clone(),
                symbol: item.symbol.clone(),
                similarity: sim,
            });
        }
    }

    matches.sort_by(|a, b| b.similarity.total_cmp(&a.similarity));
    matches.truncate(max_results);
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_exact_and_orthogonal() {
        assert!((cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn czech_stem_aware_jaccard() {
        assert!(token_jaccard("oprava timeoutu", "opravit timeouty v bridge") > 0.40);
        assert_eq!(token_jaccard("koupit mrkev", "nasadit server"), 0.0);
    }

    #[test]
    fn substring_similarity_scores_high() {
        let sim = text_similarity("timeout", "oprava timeoutu v IPC bridge");
        assert!(sim >= 0.70);
    }
}
