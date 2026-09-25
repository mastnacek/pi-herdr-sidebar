//! Reads the user's Herdr keybindings.
//!
//! Herdr has no CLI that dumps effective keybindings (`herdr config` only offers
//! `check` and `reset-keys`, and `api snapshot` carries none), so the source of
//! truth here is `config.toml` plus a table of the documented defaults. The TOML
//! subset we need is small — `[keys]` scalars and `[[keys.command]]` array
//! entries — so it is parsed directly instead of pulling in a TOML crate.
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutSource {
    /// Bound by the user in `config.toml`.
    User,
    /// Documented Herdr default.
    Default,
    /// Offered by this plugin but not bound yet: the tab shows the suggested
    /// chord so the action is discoverable before you edit `config.toml`.
    Suggested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutEntry {
    /// Resolved chord, e.g. `ctrl+b+k`.
    pub key: String,
    /// `plugin_action`, `shell`, `pane`, ... (`default` for built-ins).
    pub kind: String,
    /// Action id, plugin id, or shell command the chord runs.
    pub command: String,
    pub description: String,
    pub source: ShortcutSource,
}

#[derive(Debug, Clone, Default)]
pub struct KeyConfig {
    pub prefix: String,
    pub entries: Vec<ShortcutEntry>,
    pub config_path: Option<PathBuf>,
}

/// Documented Herdr defaults, as `(suffix, description)` after the prefix.
const DEFAULT_BINDINGS: &[(&str, &str)] = &[
    ("?", "Nápověda"),
    ("s", "Nastavení"),
    ("q", "Odpojit session"),
    ("shift+r", "Znovu načíst config"),
    ("w", "Přepínač workspace"),
    ("g", "Přejít na… (goto)"),
    ("shift+n", "Nový workspace"),
    ("shift+w", "Přejmenovat workspace"),
    ("shift+d", "Zavřít workspace"),
    ("c", "Nová záložka"),
    ("shift+t", "Přejmenovat záložku"),
    ("shift+x", "Zavřít záložku"),
    ("p", "Předchozí záložka"),
    ("n", "Další záložka"),
    ("1..9", "Přepnout na záložku 1-9"),
    ("h", "Fokus na panel vlevo"),
    ("j", "Fokus na panel dole"),
    ("k", "Fokus na panel nahoře"),
    ("l", "Fokus na panel vpravo"),
    ("tab", "Další panel"),
    ("shift+tab", "Předchozí panel"),
    ("v", "Rozdělit svisle"),
    ("minus", "Rozdělit vodorovně"),
    ("x", "Zavřít panel"),
    ("z", "Zoom panelu"),
    ("r", "Režim změny velikosti"),
    ("e", "Prohlížet scrollback"),
    ("b", "Přepnout sidebar"),
    ("shift+p", "Přejmenovat panel"),
];

const DEFAULT_PREFIX: &str = "ctrl+b";

/// Candidate `config.toml` locations, most specific first.
fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for var in ["HERDR_CONFIG", "HERDR_CONFIG_PATH"] {
        if let Some(value) = std::env::var_os(var) {
            if !value.is_empty() {
                paths.push(PathBuf::from(value));
            }
        }
    }

    #[cfg(windows)]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            paths.push(PathBuf::from(appdata).join("herdr").join("config.toml"));
        }
    }

    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(xdg).join("herdr").join("config.toml"));
    }
    if let Some(home) = crate::shared::dirs_home() {
        paths.push(home.join(".config").join("herdr").join("config.toml"));
    }
    paths
}

/// Loads user bindings and merges them with the documented defaults.
pub fn load() -> KeyConfig {
    let mut path_hit = None;
    let mut text = String::new();
    for path in config_paths() {
        if let Ok(found) = std::fs::read_to_string(&path) {
            text = found;
            path_hit = Some(path);
            break;
        }
    }

    let (configured_prefix, mut user) = parse_key_config(&text);
    let prefix = configured_prefix.unwrap_or_else(|| DEFAULT_PREFIX.to_string());

    // Resolve the `prefix+` token inside user chords so the list shows real keys.
    for entry in &mut user {
        entry.key = resolve_prefix(&entry.key, &prefix);
    }
    user.sort_by(|a, b| a.key.cmp(&b.key));

    let user_keys: Vec<String> = user.iter().map(|e| e.key.clone()).collect();
    let suggestions = suggested_entries(&prefix, &user);
    let mut entries = user;
    entries.extend(suggestions);
    entries.extend(
        default_entries(&prefix)
            .into_iter()
            .filter(|entry| !user_keys.iter().any(|k| same_chord(k, &entry.key))),
    );

    KeyConfig {
        prefix,
        entries,
        config_path: path_hit,
    }
}

/// Actions this plugin offers, with a suggested chord and description.
///
/// They are listed on the Shortcuts tab even before the user binds them (that is
/// how the standalone usage overview becomes discoverable). Once a real binding
/// exists in `config.toml`, the user entry shadows the suggestion.
///
/// The suggested suffixes are deliberately chords that Herdr does not use by
/// default — [`suggested_entries`] additionally refuses to clobber a default.
const PLUGIN_ACTIONS: &[(&str, &str, &str)] = &[
    (
        "pi.herdr-sidebar.usage-win",
        "u",
        "Pi Herdr Plugin Usage — okno s přehledem pluginů a skillů",
    ),
    (
        "pi.herdr-sidebar.edit-notes-win",
        "m",
        "Pi Herdr Notes Editor — editor SPAI poznámek",
    ),
];

/// Suggested chords for this plugin's actions that are not bound yet.
fn suggested_entries(prefix: &str, user: &[ShortcutEntry]) -> Vec<ShortcutEntry> {
    let default_chords: Vec<String> = DEFAULT_BINDINGS
        .iter()
        .map(|(suffix, _)| format!("{prefix}+{suffix}"))
        .collect();

    PLUGIN_ACTIONS
        .iter()
        .filter_map(|(action, suffix, description)| {
            let chord = format!("{prefix}+{suffix}");
            let already_bound = user.iter().any(|entry| {
                action_matches(&entry.command, action) || same_chord(&entry.key, &chord)
            });
            let clobbers_default = default_chords.iter().any(|d| same_chord(d, &chord));
            if already_bound || clobbers_default {
                return None;
            }
            Some(ShortcutEntry {
                key: chord,
                kind: "plugin_action".to_string(),
                command: (*action).to_string(),
                description: (*description).to_string(),
                source: ShortcutSource::Suggested,
            })
        })
        .collect()
}

/// Compares plugin action ids, ignoring the platform suffix (`-win`).
fn action_matches(bound: &str, action: &str) -> bool {
    let strip = |s: &str| s.trim().trim_end_matches("-win").to_lowercase();
    !bound.is_empty() && strip(bound) == strip(action)
}

fn same_chord(a: &str, b: &str) -> bool {
    normalise_chord(a) == normalise_chord(b)
}

/// Herdr accepts `prefix+n` and `ctrl+b+n` for the same chord.
fn normalise_chord(chord: &str) -> String {
    chord.trim().to_ascii_lowercase()
}

fn resolve_prefix(key: &str, prefix: &str) -> String {
    let key = key.trim();
    if key == "prefix" {
        return prefix.to_string();
    }
    match key.strip_prefix("prefix+") {
        Some(rest) => format!("{prefix}+{rest}"),
        None => key.to_string(),
    }
}

fn default_entries(prefix: &str) -> Vec<ShortcutEntry> {
    DEFAULT_BINDINGS
        .iter()
        .map(|(suffix, description)| ShortcutEntry {
            key: format!("{prefix}+{suffix}"),
            kind: "default".to_string(),
            command: String::new(),
            description: description.to_string(),
            source: ShortcutSource::Default,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Keys,
    KeyCommand,
    Other,
}

#[derive(Debug, Default)]
struct PartialEntry {
    key: String,
    kind: String,
    command: String,
    description: String,
}

/// Parses the `[keys]` and `[[keys.command]]` sections out of a Herdr config.
pub(crate) fn parse_key_config(text: &str) -> (Option<String>, Vec<ShortcutEntry>) {
    let mut prefix = None;
    let mut entries = Vec::new();
    let mut section = Section::Other;
    let mut current: Option<PartialEntry> = None;

    for raw in text.lines() {
        let line = strip_comment(raw);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("[[") {
            flush(&mut current, &mut entries);
            section = if line.starts_with("[[keys.command]]") {
                current = Some(PartialEntry::default());
                Section::KeyCommand
            } else {
                Section::Other
            };
            continue;
        }
        if line.starts_with('[') {
            flush(&mut current, &mut entries);
            section = if line.starts_with("[keys]") {
                Section::Keys
            } else {
                Section::Other
            };
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = unquote(value.trim());

        match section {
            Section::Keys => {
                if key == "prefix" && !value.is_empty() {
                    prefix = Some(value);
                }
            }
            Section::KeyCommand => {
                if let Some(entry) = current.as_mut() {
                    match key {
                        "key" => entry.key = value,
                        "type" => entry.kind = value,
                        "command" => entry.command = value,
                        "description" => entry.description = value,
                        _ => {}
                    }
                }
            }
            Section::Other => {}
        }
    }
    flush(&mut current, &mut entries);
    (prefix, entries)
}

fn flush(current: &mut Option<PartialEntry>, entries: &mut Vec<ShortcutEntry>) {
    if let Some(partial) = current.take() {
        if !partial.key.is_empty() {
            entries.push(ShortcutEntry {
                key: partial.key,
                kind: if partial.kind.is_empty() {
                    "command".to_string()
                } else {
                    partial.kind
                },
                command: partial.command,
                description: partial.description,
                source: ShortcutSource::User,
            });
        }
    }
}

/// Drops a trailing `# comment`, ignoring `#` inside quotes.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '#' => return &line[..i],
                _ => {}
            },
        }
    }
    line
}

fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if value.len() >= 2 {
        let first = bytes[0] as char;
        let last = bytes[value.len() - 1] as char;
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

#[cfg(test)]
mod tests;
