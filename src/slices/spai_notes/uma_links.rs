//! Read-only SPAI ↔ UMA relation discovery.
//!
//! The project `.uma` store is scanned only when a project's SPAI items are
//! (re)loaded. Rendering uses the cached relations and never touches the disk.
use super::note::SpaiNoteItem;
use super::storage_format::parser::unescape_yaml_scalar;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UmaLinkDirection {
    Forward,
    Reverse,
    Both,
}

impl UmaLinkDirection {
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Forward => "→",
            Self::Reverse => "←",
            Self::Both => "↔",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UmaMemoryLink {
    pub id: String,
    pub fact_type: String,
    pub title: String,
    pub direction: UmaLinkDirection,
}

impl UmaMemoryLink {
    pub fn short_id(&self) -> &str {
        &self.id[self.id.len().saturating_sub(6)..]
    }
}

#[derive(Debug)]
struct UmaFact {
    id: String,
    fact_type: String,
    title: String,
    spai_ids: Vec<String>,
}

/// Resolves all item relations from one snapshot of a project's UMA store.
pub fn resolve_memory_links(
    uma_dir: &Path,
    items: &[SpaiNoteItem],
) -> HashMap<PathBuf, Vec<UmaMemoryLink>> {
    let mut result = HashMap::new();
    if items.is_empty() {
        return result;
    }
    let facts = load_facts(uma_dir);
    if facts.is_empty() {
        return result;
    }

    let mut by_id = HashMap::new();
    let mut by_spai: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, fact) in facts.iter().enumerate() {
        by_id.insert(fact.id.as_str(), index);
        for spai_id in &fact.spai_ids {
            by_spai.entry(spai_id.as_str()).or_default().push(index);
        }
    }

    for item in items {
        let forward_ids = extract_memory_ulids(&item.body);
        let normalized_item_id = item.id.to_ascii_uppercase();
        let mut indices = Vec::new();
        for id in &forward_ids {
            if let Some(&index) = by_id.get(id.as_str()) {
                indices.push(index);
            }
        }
        if let Some(reverse) = by_spai.get(normalized_item_id.as_str()) {
            indices.extend(reverse.iter().copied());
        }
        indices.sort_unstable();
        indices.dedup();
        if indices.is_empty() {
            continue;
        }

        let mut links: Vec<_> = indices
            .into_iter()
            .map(|index| {
                let fact = &facts[index];
                let forward = forward_ids.iter().any(|id| id == &fact.id);
                let reverse = fact.spai_ids.iter().any(|id| id == &normalized_item_id);
                let direction = match (forward, reverse) {
                    (true, true) => UmaLinkDirection::Both,
                    (true, false) => UmaLinkDirection::Forward,
                    (false, true) => UmaLinkDirection::Reverse,
                    (false, false) => unreachable!("index entered through a relation"),
                };
                UmaMemoryLink {
                    id: fact.id.clone(),
                    fact_type: fact.fact_type.clone(),
                    title: fact.title.clone(),
                    direction,
                }
            })
            .collect();
        links.sort_by(|a, b| {
            a.fact_type
                .cmp(&b.fact_type)
                .then_with(|| a.title.cmp(&b.title))
                .then_with(|| a.id.cmp(&b.id))
        });
        result.insert(item.file_path.clone(), links);
    }
    result
}

/// Extracts valid, unique ULIDs from `Memory: <ULID>` text lines.
pub fn extract_memory_ulids(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in text.lines() {
        let Some(raw) = line.trim_start().strip_prefix("Memory:") else {
            continue;
        };
        let Some(token) = raw.split_whitespace().next() else {
            continue;
        };
        if let Some(id) = normalize_ulid(token) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

fn load_facts(uma_dir: &Path) -> Vec<UmaFact> {
    let Ok(type_dirs) = fs::read_dir(uma_dir) else {
        return Vec::new();
    };
    let mut facts = Vec::new();
    for type_dir in type_dirs.flatten() {
        let path = type_dir.path();
        if !path.is_dir() {
            continue;
        }
        let fallback_type = type_dir.file_name().to_string_lossy().into_owned();
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            let fallback_id = path.file_stem().and_then(|id| id.to_str()).unwrap_or("");
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            if let Some(fact) = parse_uma_fact(&content, fallback_id, &fallback_type) {
                facts.push(fact);
            }
        }
    }
    facts
}

fn parse_uma_fact(content: &str, fallback_id: &str, fallback_type: &str) -> Option<UmaFact> {
    let yaml = frontmatter(content)?;
    let mut id = None;
    let mut fact_type = None;
    let mut title = String::new();
    let mut spai_ids = Vec::new();
    let mut spai_block = false;

    for line in yaml.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if spai_block {
                if let Some(value) = line.trim().strip_prefix("- ") {
                    if let Some(spai_id) = normalize_spai_id(value) {
                        spai_ids.push(spai_id);
                    }
                }
            }
            continue;
        }
        spai_block = false;
        let line = line.trim();
        if let Some(value) = line.strip_prefix("id:") {
            id = normalize_ulid(value);
        } else if let Some(value) = line.strip_prefix("type:") {
            fact_type = Some(parse_scalar(value));
        } else if let Some(value) = line.strip_prefix("title:") {
            title = parse_scalar(value);
        } else if let Some(value) = line.strip_prefix("spai:") {
            let value = value.trim();
            if value.is_empty() {
                spai_block = true;
            } else {
                spai_ids = parse_spai_flow(value)?;
            }
        }
    }

    Some(UmaFact {
        id: id.or_else(|| normalize_ulid(fallback_id))?,
        fact_type: fact_type
            .filter(|kind| !kind.is_empty())
            .unwrap_or_else(|| fallback_type.to_string()),
        title,
        spai_ids,
    })
}

fn frontmatter(content: &str) -> Option<&str> {
    let rest = content
        .strip_prefix("---\r\n")
        .or_else(|| content.strip_prefix("---\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']).trim() == "---" {
            return Some(&rest[..offset]);
        }
        offset += line.len();
    }
    None
}

fn parse_spai_flow(raw: &str) -> Option<Vec<String>> {
    let inner = raw.strip_prefix('[')?.strip_suffix(']')?;
    Some(
        inner
            .split(',')
            .filter_map(normalize_spai_id)
            .collect::<Vec<_>>(),
    )
}

fn normalize_spai_id(raw: &str) -> Option<String> {
    let id = parse_scalar(raw).to_ascii_uppercase();
    let number = id.strip_prefix("SPAI-")?;
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(id)
}

fn normalize_ulid(raw: &str) -> Option<String> {
    let id = parse_scalar(raw).to_ascii_uppercase();
    if id.len() == 26
        && id.bytes().all(|b| {
            matches!(b,
                b'0'..=b'9' | b'A'..=b'H' | b'J'..=b'K' | b'M'..=b'N' | b'P'..=b'T' | b'V'..=b'Z'
            )
        })
    {
        Some(id)
    } else {
        None
    }
}

fn parse_scalar(raw: &str) -> String {
    let raw = raw.trim();
    let bytes = raw.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        let inner = &raw[1..raw.len() - 1];
        if bytes[0] == b'"' {
            unescape_yaml_scalar(inner)
        } else {
            inner.replace("''", "'")
        }
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
mod tests;
