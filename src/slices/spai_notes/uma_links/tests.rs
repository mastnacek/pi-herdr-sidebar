use super::*;
use crate::slices::spai_notes::storage_format::parse_spai_markdown;
use std::fs;

const FORWARD_ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const REVERSE_ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const BOTH_ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAX";

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "spai_uma_links_{tag}_{}_{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_fact(root: &Path, folder: &str, id: &str, content: &str) {
    let dir = root.join(folder);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{id}.md")), content).unwrap();
}

fn item(body: &str) -> SpaiNoteItem {
    let content = format!("---\ntype: Todo\ntitle: Task\n---\n\n# SPAI-035: Task\n{body}");
    parse_spai_markdown(&content, PathBuf::from("2026-10-14-SPAI-035-task.md")).expect("SPAI item")
}

#[test]
fn memory_lines_extract_valid_unique_ulids_only() {
    let text = format!(
        ". Task\nMemory: {FORWARD_ID}\n  Memory: {}\nMemory: {FORWARD_ID}\nMemory: invalid\nNotMemory: {REVERSE_ID}\nmemory: {REVERSE_ID}\nMemory:",
        REVERSE_ID.to_ascii_lowercase()
    );
    assert_eq!(
        extract_memory_ulids(&text),
        vec![FORWARD_ID.to_string(), REVERSE_ID.to_string()]
    );
}

#[test]
fn uma_frontmatter_parses_flow_spai_sequence() {
    let content = format!(
        "---\nid: {FORWARD_ID}\ntype: decision\ntitle: \"A \\\"linked\\\" decision\"\nspai: [\"SPAI-035\", 'SPAI-036']\n---\nbody"
    );
    let fact = parse_uma_fact(&content, "invalid", "note").expect("valid fact");
    assert_eq!(fact.id, FORWARD_ID);
    assert_eq!(fact.fact_type, "decision");
    assert_eq!(fact.title, "A \"linked\" decision");
    assert_eq!(fact.spai_ids, ["SPAI-035", "SPAI-036"]);
}

#[test]
fn uma_frontmatter_parses_block_spai_sequence_and_fallbacks() {
    let content = "---\ntitle: Reverse link\nspai:\n  - SPAI-035\n  - \"SPAI-036\"\n---\nbody";
    let fact = parse_uma_fact(content, REVERSE_ID, "pattern").expect("valid fact");
    assert_eq!(fact.id, REVERSE_ID);
    assert_eq!(fact.fact_type, "pattern");
    assert_eq!(fact.spai_ids, ["SPAI-035", "SPAI-036"]);
}

#[test]
fn missing_or_malformed_frontmatter_is_a_clean_miss() {
    assert!(parse_uma_fact("no frontmatter", FORWARD_ID, "note").is_none());
    assert!(parse_uma_fact("---\nid: x\n", FORWARD_ID, "note").is_none());
    let malformed_list = "---\ntitle: bad\nspai: [SPAI-035\n---\nbody";
    assert!(parse_uma_fact(malformed_list, FORWARD_ID, "note").is_none());
}

#[test]
fn missing_spai_field_and_invalid_ids_produce_no_reverse_ids() {
    let without = format!("---\nid: {FORWARD_ID}\ntitle: Plain\n---\nbody");
    assert!(parse_uma_fact(&without, "", "note")
        .expect("fact")
        .spai_ids
        .is_empty());

    let invalid = format!("---\nid: {FORWARD_ID}\nspai: [wrong, SPAI-x]\n---\nbody");
    assert!(parse_uma_fact(&invalid, "", "note")
        .expect("fact")
        .spai_ids
        .is_empty());
}

#[test]
fn relations_resolve_forward_reverse_and_both_from_one_store() {
    let root = temp_dir("relations");
    let uma = root.join(".uma");
    write_fact(
        &uma,
        "decision",
        FORWARD_ID,
        &format!("---\nid: {FORWARD_ID}\ntype: decision\ntitle: Forward\n---\nbody"),
    );
    write_fact(
        &uma,
        "pattern",
        REVERSE_ID,
        &format!("---\nid: {REVERSE_ID}\ntitle: Reverse\nspai: [SPAI-035]\n---\nbody"),
    );
    write_fact(
        &uma,
        "note",
        BOTH_ID,
        &format!("---\nid: {BOTH_ID}\ntitle: Both\nspai:\n  - SPAI-035\n---\nbody"),
    );
    write_fact(
        &uma,
        "note",
        "broken",
        "---\ntitle: no closing frontmatter\n",
    );

    let selected = item(&format!(
        ". Task\nMemory: {FORWARD_ID}\nMemory: {BOTH_ID}\nMemory: 01ARZ3NDEKTSV4RRFFQ69G5FAZ"
    ));
    // A distinct file path is what makes this a different item: entering it
    // under the same path would overwrite the selected item's entry.
    let unrelated = parse_spai_markdown(
        "---\ntype: Todo\ntitle: Other\n---\n\n# SPAI-099: Other\n. Other\nMemory: 01ARZ3NDEKTSV4RRFFQ69G5FAZ",
        PathBuf::from("2026-10-14-SPAI-099-other.md"),
    )
    .expect("SPAI item");
    let resolved = resolve_memory_links(&uma, &[selected.clone(), unrelated]);
    let links = resolved.get(&selected.file_path).expect("links");
    assert_eq!(links.len(), 3);

    let forward = links.iter().find(|link| link.id == FORWARD_ID).unwrap();
    assert_eq!(forward.direction, UmaLinkDirection::Forward);
    assert_eq!(forward.short_id(), "9G5FAV");
    assert_eq!(forward.fact_type, "decision");

    let reverse = links.iter().find(|link| link.id == REVERSE_ID).unwrap();
    assert_eq!(reverse.direction, UmaLinkDirection::Reverse);
    assert_eq!(reverse.fact_type, "pattern");

    let both = links.iter().find(|link| link.id == BOTH_ID).unwrap();
    assert_eq!(both.direction, UmaLinkDirection::Both);
    assert_eq!(resolved.len(), 1, "unrelated item has no map entry");

    fs::remove_dir_all(root).ok();
}

#[test]
fn absent_store_and_unreadable_shapes_render_no_links() {
    let root = temp_dir("missing_store");
    let selected = item(&format!(". Task\nMemory: {FORWARD_ID}"));
    assert!(resolve_memory_links(&root.join("missing"), &[selected.clone()]).is_empty());

    let uma = root.join(".uma");
    fs::create_dir_all(&uma).unwrap();
    fs::write(uma.join("not-a-directory"), "junk").unwrap();
    assert!(resolve_memory_links(&uma, &[selected]).is_empty());
    fs::remove_dir_all(root).ok();
}
