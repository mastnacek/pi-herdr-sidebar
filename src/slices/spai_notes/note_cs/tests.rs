use crate::slices::spai_notes::storage_format::{format_spai_markdown, parse_spai_markdown};
use std::path::PathBuf;

const FILE: &str = "---\ntype: Todo\ntitle: \"Fix login\"\ntimestamp: 2026-11-20 10:00:00\nstatus: todo\nsource: spai\nspai_symbol: '.'\nlang: en\ntitle_cs: \"Oprava \\\"přihlášení\\\"\"\npending: [embed]\n---\n\n# SPAI-001: Fix login\n\n. Fix login\n\n## Původní znění\n\nOprava přihlášení\n";

fn item() -> crate::slices::spai_notes::note::SpaiNoteItem {
    parse_spai_markdown(FILE, PathBuf::from("2026-11-20-SPAI-001-fix-login.md")).unwrap()
}

#[test]
fn shows_the_czech_title_and_falls_back() {
    let it = item();
    assert_eq!(it.display_title(), "Oprava \"přihlášení\"");
    let mut plain = item();
    plain.extra_frontmatter.retain(|(k, _)| k != "title_cs");
    assert_eq!(plain.display_title(), "Fix login");
}

#[test]
fn parser_and_formatter_keep_lang_title_cs_pending_and_the_original() {
    let out = format_spai_markdown(&item());
    for needle in ["lang: en", "title_cs: \"Oprava \\\"přihlášení\\\"\"", "pending: [embed]", "## Původní znění", "Oprava přihlášení"] {
        assert!(out.contains(needle), "{needle} lost:\n{out}");
    }
}

#[test]
fn unchanged_title_keeps_title_cs() {
    let mut it = item();
    let old = it.title.clone();
    it.on_title_changed(&old);
    assert!(it.title_cs().is_some());
    assert!(format_spai_markdown(&it).contains("pending: [embed]"));
}

#[test]
fn changed_title_drops_title_cs_and_adds_translate_cs() {
    let mut it = item();
    let old = it.title.clone();
    it.title = "Fix sign-in".to_string();
    it.on_title_changed(&old);
    assert!(it.title_cs().is_none());
    assert_eq!(it.display_title(), "Fix sign-in");
    let out = format_spai_markdown(&it);
    assert!(out.contains("pending: [embed, translate_cs]"), "{out}");
    assert!(!out.contains("title_cs:"), "{out}");
    assert!(out.contains("lang: en"));
    // A second edit does not add it twice.
    let old = it.title.clone();
    it.title = "Fix sign in".to_string();
    it.on_title_changed(&old);
    assert_eq!(format_spai_markdown(&it).matches("translate_cs").count(), 1);
}

#[test]
fn a_record_without_pending_gets_one() {
    let mut it = item();
    it.extra_frontmatter.retain(|(k, _)| k != "pending");
    let old = it.title.clone();
    it.title = "Other".to_string();
    it.on_title_changed(&old);
    assert!(format_spai_markdown(&it).contains("pending: [translate_cs]"));
}

#[test]
fn edited_czech_title_goes_to_title_cs_and_asks_for_retitle_en() {
    let mut it = item();
    assert!(it.retitle_cs("Nové \"znění\""));
    assert_eq!(it.title, "Fix login");
    assert_eq!(it.display_title(), "Nové \"znění\"");
    let out = format_spai_markdown(&it);
    assert!(out.contains("pending: [embed, retitle_en]"), "{out}");
    assert!(it.retitle_cs("Jinak"));
    assert_eq!(format_spai_markdown(&it).matches("retitle_en").count(), 1);
    let mut plain = item();
    plain.extra_frontmatter.retain(|(k, _)| k != "title_cs");
    assert!(!plain.retitle_cs("X"));
}
