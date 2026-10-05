use super::*;
use crate::slices::spai_notes::note::SpaiType;
use std::path::PathBuf;

fn note(path: &str) -> PathBuf {
    PathBuf::from(path)
}

#[test]
fn inline_tags_are_parsed_and_stripped() {
    assert_eq!(
        parse_inline_tags("[ai, chat, fix]"),
        vec!["ai", "chat", "fix"]
    );
    assert_eq!(parse_inline_tags("ai, chat"), vec!["ai", "chat"]);
    assert_eq!(parse_inline_tags("solo"), vec!["solo"]);
    assert_eq!(parse_inline_tags("['a', \"b\"]"), vec!["a", "b"]);
    assert!(parse_inline_tags("[]").is_empty());
}

#[test]
fn frontmatter_tags_survive_parse_format_round_trip() {
    let src = "---\ntype: Todo\ntitle: \"T\"\ntimestamp: 2026-09-09 20:19:09\nstatus: done\nsource: pi-spai\ntags: [ai, chat, fix]\nfacets:\n  project: skoly\nspai_symbol: 'x'\n---\n\n# SPAI-009: T\n\nx T\n";
    let item = parse_spai_markdown(src, note("2026-09-09-SPAI-009-t.md")).unwrap();
    assert_eq!(item.tags, vec!["ai", "chat", "fix"]);

    let round_tripped = format_spai_markdown(&item);
    assert!(
        round_tripped.contains("tags: [ai, chat, fix]"),
        "tags dropped on save: {round_tripped}"
    );
    let again = parse_spai_markdown(&round_tripped, note("2026-09-09-SPAI-009-t.md")).unwrap();
    assert_eq!(again.tags, vec!["ai", "chat", "fix"]);
}

#[test]
fn multiline_yaml_tag_list_is_parsed() {
    let src = "---\ntype: Todo\ntitle: \"T\"\ntags:\n  - ai\n  - chat\nstatus: todo\n---\n\n# SPAI-001: T\n\n. T\n";
    let item = parse_spai_markdown(&src, note("2026-01-01-SPAI-001-t.md")).unwrap();
    assert_eq!(item.tags, vec!["ai", "chat"]);
    assert_eq!(item.kind, SpaiType::Todo);
}

#[test]
fn note_without_tags_stays_tagless() {
    let src = "---\ntype: Todo\ntitle: \"T\"\nstatus: todo\n---\n\n# SPAI-001: T\n\n. T\n";
    let item = parse_spai_markdown(&src, note("2026-01-01-SPAI-001-t.md")).unwrap();
    assert!(item.tags.is_empty());
    assert!(!format_spai_markdown(&item).contains("tags:"));
}

#[test]
fn unknown_frontmatter_keys_survive_the_round_trip() {
    // `source` plus a foreign tool's key must not be dropped by a status
    // cycle or an edit, which rewrite the whole frontmatter.
    let src = "---\ntype: Todo\ntitle: \"T\"\ntimestamp: 2026-09-09 20:19:09\nstatus: todo\nsource: pi-spai\ncustom_key: some-value\nspai_symbol: '.'\n---\n\n# SPAI-001: T\n\n. T\n";
    let item = parse_spai_markdown(&src, note("2026-01-01-SPAI-001-t.md")).unwrap();
    assert!(item
        .extra_frontmatter
        .iter()
        .any(|(k, v)| k == "custom_key" && v == "some-value"));

    let round_tripped = format_spai_markdown(&item);
    assert!(
        round_tripped.contains("custom_key: some-value"),
        "foreign frontmatter key dropped: {round_tripped}"
    );
    // The file's own `source` is kept instead of re-defaulted.
    let once = round_tripped.matches("source:").count();
    assert_eq!(once, 1, "source duplicated or lost: {round_tripped}");

    let again = parse_spai_markdown(&round_tripped, note("2026-01-01-SPAI-001-t.md")).unwrap();
    assert!(again
        .extra_frontmatter
        .iter()
        .any(|(k, v)| k == "custom_key" && v == "some-value"));
}

#[test]
fn title_escapes_quotes_and_backslashes_both_ways() {
    // No SPAI heading in the body, so the title comes from the frontmatter
    // (where the formatter escapes and the parser unescapes).
    let bs = char::from(0x5C); // backslash, kept out of the source literal
    let title = format!("a {bs} \"q\" b");
    let escaped = title
        .replace(&bs.to_string(), &bs.to_string().repeat(2))
        .replace('"', &format!("{bs}\"")); 
    let src = format!(
        "---
type: Todo
title: \"{}\"
timestamp: 1
status: todo
---

. B
",
        escaped
    );
    let item = parse_spai_markdown(&src, note("2026-01-01-SPAI-001-t.md")).unwrap();
    assert_eq!(item.title, title, "frontmatter title unescaped");

    let written = format_spai_markdown(&item);
    let again = parse_spai_markdown(&written, note("2026-01-01-SPAI-001-t.md")).unwrap();
    assert_eq!(again.title, title, "title changed across round trip");
}

#[test]
fn frontmatter_end_ignores_dashes_inside_values_and_longer_lines() {
    // `----` is not a terminator, and `---` inside a value is not either.
    let src = "---\ntitle: \"a---b\"\n----\nstatus: todo\n---\n\n# SPAI-001: T\n\n. T\n";
    let item = parse_spai_markdown(&src, note("2026-01-01-SPAI-001-t.md")).unwrap();
    assert_eq!(item.status.as_str(), "todo", "frontmatter ended too early");
    assert_eq!(item.body.trim_end(), ". T");
}

#[test]
fn header_is_not_confused_with_its_text_elsewhere_in_the_body() {
    // The same text as the header line appears earlier in the body; the body
    // must start after the real header, not after the earlier occurrence.
    let src = "---\nstatus: todo\n---\n\nbody mentions # SPAI-001: T before\n# SPAI-001: T\n\n. T\n";
    let item = parse_spai_markdown(&src, note("2026-01-01-SPAI-001-t.md")).unwrap();
    assert!(
        !item.body.contains("# SPAI-001: T"),
        "header left in the body: {}",
        item.body
    );
    assert!(item.body.contains(". T"));
}

#[test]
fn missing_id_falls_back_to_a_unique_marker_not_spai_001() {
    let src = "no frontmatter\n\n# Just a heading\n\n. T\n";
    let item = parse_spai_markdown(src, note("random-name.md")).unwrap();
    assert_eq!(item.id, "SPAI-???", "fallback id would collide with real SPAI-001");
}

#[test]
fn body_prefix_update_preserves_trailing_newline_and_crlf() {
    use crate::slices::spai_notes::note::SpaiStatus;

    let lf = ". task\n";
    assert_eq!(
        update_body_status_prefix(lf, SpaiStatus::Done),
        "x task\n",
        "trailing newline kept"
    );

    let crlf = ". task\r\nsecond line\r\n";
    let updated = update_body_status_prefix(crlf, SpaiStatus::Working);
    assert_eq!(updated, "/ task\r\nsecond line\r\n", "CRLF style kept: {updated:?}");
}
