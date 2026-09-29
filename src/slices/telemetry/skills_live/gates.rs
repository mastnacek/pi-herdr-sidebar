//! Compliance gates over mutated files — pure functions of (path, code) so the
//! Skills face can score the same invariants the TS auditor enforces.

use serde_json::Value;

use super::super::skills::ComplianceCheck;

/// Compliance gates over a mutated file.
pub(super) fn run_gates(path: &str, code: &str, _ts: u64, out: &mut Vec<ComplianceCheck>) {
    if code.is_empty() && !path.ends_with("package.json") {
        return;
    }
    let code = strip_comments(code);
    let push = |check: ComplianceCheck, out: &mut Vec<ComplianceCheck>| {
        if let Some(existing) = out
            .iter_mut()
            .find(|c| c.rule == check.rule && c.label == check.label)
        {
            *existing = check;
        } else {
            out.push(check);
        }
    };

    package_json_gates(path, &code, &push, out);
    string_enum_gate(&code, &push, out);
    error_throw_gate(&code, &push, out);
    trailing_space_gate(&code, &push, out);
    lifecycle_cleanup_gate(&code, &push, out);
    ui_mode_guard_gate(&code, &push, out);
}

/// package.json gates: peer-deps + manifest hygiene.
fn package_json_gates(
    path: &str,
    code: &str,
    push: &impl Fn(ComplianceCheck, &mut Vec<ComplianceCheck>),
    out: &mut Vec<ComplianceCheck>,
) {
    if !path.ends_with("package.json") {
        return;
    }
    let Ok(v) = serde_json::from_str::<Value>(code) else {
        return;
    };
    let core_in_deps = v
        .get("dependencies")
        .and_then(Value::as_object)
        .map(|d| d.keys().any(|k| k.starts_with("@earendil-works")))
        .unwrap_or(false);
    if core_in_deps {
        push(ComplianceCheck {
            rule: "peer-deps".into(),
            label: "Peer Dependencies".into(),
            status: "fail".into(),
            details: "Core @earendil-works package in dependencies — must stay in peerDependencies."
                .into(),
            ..Default::default()
        }, out);
    } else if v.get("dependencies").is_some() {
        push(
            ComplianceCheck {
                rule: "peer-deps".into(),
                label: "Peer Dependencies".into(),
                status: "pass".into(),
                details: "No core package bundled in dependencies".into(),
                ..Default::default()
            },
            out,
        );
    }
    if v.get("type").and_then(Value::as_str) != Some("module") {
        push(
            ComplianceCheck {
                rule: "manifest-hygiene".into(),
                label: "ESM Manifest".into(),
                status: "warn".into(),
                details: "package.json missing \"type\": \"module\".".into(),
                ..Default::default()
            },
            out,
        );
    }
}

/// StringEnum schema rule: `Type.Union` of `Type.Literal` breaks Google Gemini.
fn string_enum_gate(
    code: &str,
    push: &impl Fn(ComplianceCheck, &mut Vec<ComplianceCheck>),
    out: &mut Vec<ComplianceCheck>,
) {
    let has_union = code.contains("Type.Union") && code.contains("Type.Literal");
    let has_string_enum = code.contains("StringEnum(");
    if !(code.contains("registerTool") || code.contains("Type.Object")) {
        return;
    }
    let status = if has_union {
        "fail"
    } else if has_string_enum {
        "pass"
    } else {
        return;
    };
    push(
        ComplianceCheck {
            rule: "string-enum".into(),
            label: "StringEnum Schema Rule".into(),
            status: status.into(),
            details: if has_union {
                "Type.Union of Type.Literal breaks Google Gemini — use StringEnum([...]).".into()
            } else {
                "StringEnum used for enum parameters (Gemini compatible).".into()
            },
            ..Default::default()
        },
        out,
    );
}

/// Error reporting contract: tool errors must throw, not return isError.
fn error_throw_gate(
    code: &str,
    push: &impl Fn(ComplianceCheck, &mut Vec<ComplianceCheck>),
    out: &mut Vec<ComplianceCheck>,
) {
    if !(code.contains("registerTool") && code.contains("execute")) {
        return;
    }
    let returns_error =
        code.contains("isError: true") || code.contains("error: \"") || code.contains("error: '");
    let throws = code.contains("throw new Error(");
    if returns_error && !throws {
        push(ComplianceCheck {
            rule: "error-throw".into(),
            label: "Error Reporting Contract".into(),
            status: "warn".into(),
            details: "Tool returns an error payload instead of throwing Error (isError is only set via throw).".into(),
            ..Default::default()
        }, out);
    } else if throws {
        push(
            ComplianceCheck {
                rule: "error-throw".into(),
                label: "Error Reporting Contract".into(),
                status: "pass".into(),
                details: "Tool errors reported via throw new Error().".into(),
                ..Default::default()
            },
            out,
        );
    }
}

/// Trailing Space Contract: markers belong in label/description, not value.
fn trailing_space_gate(
    code: &str,
    push: &impl Fn(ComplianceCheck, &mut Vec<ComplianceCheck>),
    out: &mut Vec<ComplianceCheck>,
) {
    for line in code.lines() {
        let trimmed = line.trim_start();
        if let Some(pos) = trimmed.find("value:") {
            let rest = &trimmed[pos..];
            if rest.contains('✓') || rest.contains('●') || rest.contains('○') {
                push(ComplianceCheck {
                    rule: "trailing-space".into(),
                    label: "Completion Marker Guard".into(),
                    status: "fail".into(),
                    details: "Marker found inside item.value — value is inserted verbatim; markers belong in label/description.".into(),
                    ..Default::default()
                }, out);
                break;
            }
        }
    }
}

/// Lifecycle cleanup: every `pi.on` must be tracked for unsubscribe.
fn lifecycle_cleanup_gate(
    code: &str,
    push: &impl Fn(ComplianceCheck, &mut Vec<ComplianceCheck>),
    out: &mut Vec<ComplianceCheck>,
) {
    if !code.contains("pi.on(") {
        return;
    }
    let status = if code.contains("unsubscribe")
        || code.contains("track(")
        || code.contains("unsubscribers")
    {
        "pass"
    } else {
        "warn"
    };
    push(
        ComplianceCheck {
            rule: "lifecycle-cleanup".into(),
            label: "Lifecycle Listener Cleanup".into(),
            status: status.into(),
            details: if status == "pass" {
                "Listeners tracked for session_shutdown unsubscribe.".into()
            } else {
                "pi.on() result not stored — listener leaks on /reload.".into()
            },
            ..Default::default()
        },
        out,
    );
}

/// UI-mode guard: TUI-only state must be guarded.
fn ui_mode_guard_gate(
    code: &str,
    push: &impl Fn(ComplianceCheck, &mut Vec<ComplianceCheck>),
    out: &mut Vec<ComplianceCheck>,
) {
    if !code.contains("custom(") {
        return;
    }
    let status = if code.contains("hasUI") || code.contains("canOverlay") {
        "pass"
    } else {
        "warn"
    };
    push(
        ComplianceCheck {
            rule: "ui-mode-guard".into(),
            label: "UI Mode Guard".into(),
            status: status.into(),
            details: if status == "pass" {
                "TUI state guarded by hasUI/canOverlay.".into()
            } else {
                "custom() without a hasUI guard breaks headless sessions.".into()
            },
            ..Default::default()
        },
        out,
    );
}

/// Comment-blind scan: removes `//` line comments and `/* */` blocks while
/// respecting string literals (a `//` inside `"https://…"` is content).
pub(super) fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                // Line comment: skip to end of line.
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                while let Some(n) = chars.next() {
                    if n == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        break;
                    }
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    out
}
