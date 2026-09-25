//! Unit tests for the Herdr keybinding reader.
use super::*;

const USER_CONFIG: &str = r#"
onboarding = false
[update]
channel = "preview"

[[keys.command]]
key = "prefix+k"
type = "plugin_action"
command = "spai.ledger.open-popup"
description = "Open SPAI Kanban Board Popup"

[[keys.command]]
key = "prefix+p"   # trailing comment
type = "plugin_action"
command = "pi.herdr-sidebar.popup-win"
description = "Open Pi Sidebar Popup"
"#;

#[test]
fn parses_plugin_action_bindings() {
    let (prefix, entries) = parse_key_config(USER_CONFIG);
    assert_eq!(prefix, None);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "prefix+k");
    assert_eq!(entries[0].kind, "plugin_action");
    assert_eq!(entries[0].command, "spai.ledger.open-popup");
    assert_eq!(entries[1].command, "pi.herdr-sidebar.popup-win");
    assert_eq!(entries[1].source, ShortcutSource::User);
}

#[test]
fn strips_comments_but_not_inside_quotes() {
    assert_eq!(strip_comment("a = 1 # tail").trim(), "a = 1");
    assert_eq!(
        strip_comment(r#"key = "a#b" # tail"#).trim(),
        r#"key = "a#b""#
    );
    assert_eq!(strip_comment("key = 1").trim(), "key = 1");
    // An unterminated quote swallows the `#`, so nothing is stripped.
    assert_eq!(strip_comment(r#"key = "a#b"#), r#"key = "a#b"#);
}

#[test]
fn reads_custom_prefix_and_resolves_chords() {
    let text = "[keys]\nprefix = \"f12\"\n\n[[keys.command]]\nkey = \"prefix+z\"\ncommand = \"plugin.thing\"\n";
    let (prefix, entries) = parse_key_config(text);
    assert_eq!(prefix.as_deref(), Some("f12"));
    assert_eq!(resolve_prefix(&entries[0].key, "f12"), "f12+z");
}

#[test]
fn defaults_are_suffixed_with_the_prefix() {
    let entries = default_entries("ctrl+b");
    assert!(entries.iter().any(|e| e.key == "ctrl+b+c"));
    assert!(entries.iter().all(|e| e.source == ShortcutSource::Default));
}

#[test]
fn user_bindings_shadow_defaults_including_prefix_notation() {
    let (_, user) = parse_key_config(USER_CONFIG);
    let resolved: Vec<ShortcutEntry> = user
        .into_iter()
        .map(|mut e| {
            e.key = resolve_prefix(&e.key, "ctrl+b");
            e
        })
        .collect();
    assert!(resolved.iter().any(|e| e.key == "ctrl+b+p"));

    // Mirrors the merge in `load`: a user chord hides the default with the
    // same (case-insensitive) chord.
    let user_keys: Vec<String> = resolved.iter().map(|e| e.key.clone()).collect();
    let merged: Vec<ShortcutEntry> = default_entries("ctrl+b")
        .into_iter()
        .filter(|d| !user_keys.iter().any(|k| same_chord(k, &d.key)))
        .collect();
    assert!(
        !merged.iter().any(|e| e.key == "ctrl+b+p"),
        "user binding must shadow the default prefix+p (previous tab)"
    );
    assert!(
        merged.iter().any(|e| e.key == "ctrl+b+n"),
        "untouched defaults survive"
    );
    assert!(
        same_chord("CTRL+B+P", "ctrl+b+p"),
        "comparison ignores case"
    );
}

#[test]
fn suggests_unbound_plugin_actions_but_never_clobbers_a_default() {
    // USER_CONFIG binds prefix+k and prefix+p only.
    let (_, user) = parse_key_config(USER_CONFIG);
    let resolved: Vec<ShortcutEntry> = user
        .into_iter()
        .map(|mut e| {
            e.key = resolve_prefix(&e.key, "ctrl+b");
            e
        })
        .collect();

    let suggestions = suggested_entries("ctrl+b", &resolved);
    let keys: Vec<&str> = suggestions.iter().map(|e| e.key.as_str()).collect();

    assert!(
        keys.contains(&"ctrl+b+u"),
        "usage overview should be discoverable: {keys:?}"
    );
    assert!(
        keys.contains(&"ctrl+b+m"),
        "notes editor should be suggested"
    );
    assert!(
        suggestions
            .iter()
            .all(|e| e.source == ShortcutSource::Suggested),
        "suggestions must be marked as such, not as user bindings"
    );
    // Chords Herdr already uses by default must never be suggested over.
    for taken in ["ctrl+b+c", "ctrl+b+p", "ctrl+b+n", "ctrl+b+b", "ctrl+b+r"] {
        assert!(
            !keys.contains(&taken),
            "{taken} is a Herdr default and must not be suggested"
        );
    }
}

#[test]
fn an_already_bound_plugin_action_is_not_suggested_again() {
    let bound = vec![ShortcutEntry {
        key: "ctrl+b+u".to_string(),
        kind: "plugin_action".to_string(),
        command: "pi.herdr-sidebar.usage-win".to_string(),
        description: String::new(),
        source: ShortcutSource::User,
    }];
    let suggestions = suggested_entries("ctrl+b", &bound);
    assert!(
        !suggestions.iter().any(|e| e.key == "ctrl+b+u"),
        "a bound action must not be suggested again"
    );
    // ... and the platform suffix is ignored when matching action ids.
    assert!(action_matches(
        "pi.herdr-sidebar.usage-win",
        "pi.herdr-sidebar.usage"
    ));
    assert!(!action_matches("", "pi.herdr-sidebar.usage"));
    assert!(!action_matches(
        "pi.herdr-sidebar.toggle",
        "pi.herdr-sidebar.usage"
    ));
}
