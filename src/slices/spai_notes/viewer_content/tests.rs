use super::super::uma_links::UmaLinkDirection;
use super::*;
use crate::slices::spai_notes::storage_format::parse_spai_markdown;
use std::path::PathBuf;

fn item(body: &str) -> SpaiNoteItem {
    let content = format!("---\ntype: Todo\ntitle: Task\n---\n\n# SPAI-035: Task\n{body}");
    parse_spai_markdown(&content, PathBuf::from("2026-10-14-SPAI-035-task.md")).unwrap()
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

fn link(id: &str, fact_type: &str, title: &str, direction: UmaLinkDirection) -> UmaMemoryLink {
    UmaMemoryLink {
        id: id.to_string(),
        fact_type: fact_type.to_string(),
        title: title.to_string(),
        direction,
    }
}

#[test]
fn links_section_renders_short_id_type_and_title() {
    let item = item(". Task\n");
    let links = vec![link(
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "decision",
        "Keep the stores separate",
        UmaLinkDirection::Forward,
    )];
    let lines = format_viewer_content(&item, &links, 60);
    let rendered: Vec<String> = lines.iter().map(line_text).collect();

    assert!(
        rendered.iter().any(|line| line.contains("Vazby / Links:")),
        "section header missing: {rendered:?}"
    );
    let relation = rendered
        .iter()
        .find(|line| line.contains("9G5FAV"))
        .expect("relation line");
    assert!(relation.contains('→'), "forward glyph missing: {relation}");
    assert!(
        relation.contains("[decision]"),
        "fact type missing: {relation}"
    );
    assert!(
        relation.contains("Keep the stores separate"),
        "title missing: {relation}"
    );
}

#[test]
fn no_links_means_no_empty_section() {
    let item = item(". Task\n");
    let rendered: Vec<String> = format_viewer_content(&item, &[], 60)
        .iter()
        .map(line_text)
        .collect();
    assert!(
        !rendered.iter().any(|line| line.contains("Vazby / Links")),
        "an empty store must not add UI noise: {rendered:?}"
    );
}

#[test]
fn long_link_titles_are_truncated_to_the_available_width() {
    let item = item(". Task\n");
    let links = vec![link(
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "a-very-long-fact-type-name",
        &"x".repeat(200),
        UmaLinkDirection::Both,
    )];
    let lines = format_viewer_content(&item, &links, 40);
    let relation = lines
        .iter()
        .find(|line| line_text(line).contains('↔'))
        .expect("relation line");

    assert!(
        line_text(relation).contains("..."),
        "oversized title must be ellipsized: {:?}",
        line_text(relation)
    );
    assert!(
        relation.width() <= 40,
        "rendered line must fit the pane: {} cells in {:?}",
        relation.width(),
        line_text(relation)
    );
}

#[test]
fn missing_title_still_renders_a_named_row() {
    let item = item(". Task\n");
    let links = vec![link(
        "01ARZ3NDEKTSV4RRFFQ69G5FAW",
        "note",
        "",
        UmaLinkDirection::Reverse,
    )];
    let rendered: Vec<String> = format_viewer_content(&item, &links, 60)
        .iter()
        .map(line_text)
        .collect();
    let relation = rendered
        .iter()
        .find(|line| line.contains("9G5FAW"))
        .expect("relation line");
    assert!(
        relation.contains("Bez názvu"),
        "fallback title missing: {relation}"
    );
}
