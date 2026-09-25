//! Resolves and launches the user's external editor for a SPAI note file
//! (`E` shortcut). Honours `$VISUAL` then `$EDITOR`, with a platform default.
use std::path::Path;
use std::process::{Command, ExitStatus};

#[derive(Debug, Clone)]
pub struct EditorCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl EditorCommand {
    pub fn display(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

/// Resolves the editor to spawn, or `None` when nothing usable is configured.
pub fn resolve_editor() -> Option<EditorCommand> {
    for var in ["VISUAL", "EDITOR"] {
        if let Ok(value) = std::env::var(var) {
            if let Some(cmd) = parse_editor(value.trim()) {
                return Some(cmd);
            }
        }
    }
    platform_default()
}

/// Parses a shell-ish editor string into program + args (supports quotes).
pub fn parse_editor(value: &str) -> Option<EditorCommand> {
    if value.is_empty() {
        return None;
    }
    let mut parts = split_args(value);
    if parts.is_empty() {
        return None;
    }
    let program = parts.remove(0);
    Some(EditorCommand {
        program,
        args: parts,
    })
}

fn split_args(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for c in value.chars() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else {
                    current.push(c);
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c.is_whitespace() {
                    if !current.is_empty() {
                        out.push(std::mem::take(&mut current));
                    }
                } else {
                    current.push(c);
                }
            }
        }
    }

    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(windows)]
fn platform_default() -> Option<EditorCommand> {
    Some(EditorCommand {
        program: "notepad".to_string(),
        args: Vec::new(),
    })
}

#[cfg(not(windows))]
fn platform_default() -> Option<EditorCommand> {
    for candidate in ["nano", "vim", "vi"] {
        if command_exists(candidate) {
            return Some(EditorCommand {
                program: candidate.to_string(),
                args: Vec::new(),
            });
        }
    }
    None
}

#[cfg(not(windows))]
fn command_exists(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(program).is_file())
}

/// Runs the editor on `path` and waits for it to exit.
pub fn run_editor(editor: &EditorCommand, path: &Path) -> std::io::Result<ExitStatus> {
    Command::new(&editor.program)
        .args(&editor.args)
        .arg(path)
        .status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_program() {
        let cmd = parse_editor("vim").unwrap();
        assert_eq!(cmd.program, "vim");
        assert!(cmd.args.is_empty());
    }

    #[test]
    fn parses_program_with_args() {
        let cmd = parse_editor("code --wait").unwrap();
        assert_eq!(cmd.program, "code");
        assert_eq!(cmd.args, vec!["--wait"]);
    }

    #[test]
    fn parses_quoted_path() {
        let cmd = parse_editor("\"C:/Program Files/Editor/ed.exe\" --new-window").unwrap();
        assert_eq!(cmd.program, "C:/Program Files/Editor/ed.exe");
        assert_eq!(cmd.args, vec!["--new-window"]);
    }

    #[test]
    fn empty_editor_is_rejected() {
        assert!(parse_editor("").is_none());
        assert!(parse_editor("   ").is_none());
    }
}
