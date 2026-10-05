use super::*;
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
    let item = parse_spai_markdown(src, note("2026-01-01-SPAI-001-t.md")).unwrap();
    assert_eq!(item.tags, vec!["ai", "chat"]);
    assert_eq!(item.kind, SpaiType::Todo);
}

#[test]
fn note_without_tags_stays_tagless() {
    let src = "---\ntype: Todo\ntitle: \"T\"\nstatus: todo\n---\n\n# SPAI-001: T\n\n. T\n";
    let item = parse_spai_markdown(src, note("2026-01-01-SPAI-001-t.md")).unwrap();
    assert!(item.tags.is_empty());
    assert!(!format_spai_markdown(&item).contains("tags:"));
}
