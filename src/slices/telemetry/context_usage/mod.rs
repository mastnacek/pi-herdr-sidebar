//! Context-window token accounting that mirrors pi's own `getContextUsage()`.
//!
//! pi derives context usage from the *session projection*
//! (`pi-coding-agent/dist/core/agent-session.js`, `dist/core/compaction/compaction.js`):
//!
//! * the last **valid** assistant usage wins — aborted, errored and all-zero
//!   usages are not valid;
//! * after a `compaction` with no later usage pi reports the context as
//!   *unknown* until the next model response;
//! * messages after the last usage are estimated with the `chars / 4` heuristic,
//!   with the latest `context_edit` per target applied.
//!
//! One deliberate deviation: pi also distrusts a usage that predates a bare
//! `context_edit` and then re-estimates the *whole* projected transcript —
//! including the system prompt, which an out-of-process sidecar cannot see.
//! Rather than fabricate that number (or blank the gauge on every context-mode
//! edit), the last real measurement is kept and only the trailing estimate is
//! adjusted. It self-corrects on the next turn, and compaction — the case where
//! pi truly reports `unknown` — is still reported as unknown.
//!
//! Everything here works with or without context-mode: sessions without
//! `context_edit` entries simply never adjust anything, which is the
//! pre-existing behaviour.

use std::collections::HashMap;

use super::Entry;

/// pi counts one image block as this many characters (`ESTIMATED_IMAGE_CHARS`).
const ESTIMATED_IMAGE_CHARS: usize = 4800;

/// Context tokens as pi would report them.
///
/// `None` means pi itself would report `unknown` — no valid usage yet, or a
/// compaction that no assistant has responded after.
pub fn estimate_context_tokens(entries: &[Entry]) -> Option<u64> {
    let (usage_index, tokens) = last_valid_usage(entries)?;

    if let Some(compaction) = entries.iter().rposition(|e| e.kind == "compaction") {
        if usage_index < compaction {
            return None;
        }
    }

    Some(tokens.saturating_add(trailing_tokens(entries, usage_index)))
}

/// Last assistant message whose usage pi would accept, with its entry index.
fn last_valid_usage(entries: &[Entry]) -> Option<(usize, u64)> {
    for (index, entry) in entries.iter().enumerate().rev() {
        if entry.kind != "message" {
            continue;
        }
        let Some(message) = entry.message.as_ref() else {
            continue;
        };
        if message.role.as_deref() != Some("assistant") {
            continue;
        }
        if matches!(
            message.stop_reason.as_deref(),
            Some("aborted") | Some("error")
        ) {
            continue;
        }
        let Some(usage) = message.usage.as_ref() else {
            continue;
        };
        let tokens = usage_context_tokens(usage);
        if tokens > 0 {
            return Some((index, tokens));
        }
    }
    None
}

/// pi's `calculateContextTokens`: `totalTokens` when present, else the parts.
fn usage_context_tokens(usage: &super::Usage) -> u64 {
    if usage.total_tokens > 0 {
        usage.total_tokens
    } else {
        usage.input + usage.output + usage.cache_read + usage.cache_write
    }
}

/// Estimated tokens for every context-producing entry after `after_index`,
/// with post-usage `context_edit` overrides applied.
fn trailing_tokens(entries: &[Entry], after_index: usize) -> u64 {
    let overrides = edit_overrides(entries, after_index);
    entries
        .iter()
        .enumerate()
        .skip(after_index + 1)
        .map(|(index, entry)| estimate_entry_tokens(entry, overrides.get(&index)))
        .sum()
}

enum Override {
    /// `replacement: null` — the target no longer contributes to context.
    Omit,
    /// `replacement: { content: ... }` — only the content is swapped.
    Replace(serde_json::Value),
}

/// Map entry index -> override for every target edited after `after_index`.
fn edit_overrides(entries: &[Entry], after_index: usize) -> HashMap<usize, Override> {
    let mut index_by_id: HashMap<&str, usize> = HashMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if let Some(id) = entry.id.as_deref() {
            index_by_id.insert(id, index);
        }
    }

    let mut overrides = HashMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if index <= after_index || entry.kind != "context_edit" {
            continue;
        }
        let Some(target) = entry.target_id.as_deref() else {
            continue;
        };
        let Some(target_index) = index_by_id.get(target).copied() else {
            continue;
        };
        let override_value = match entry.replacement.as_ref() {
            None | Some(serde_json::Value::Null) => Override::Omit,
            Some(value) => Override::Replace(value.clone()),
        };
        overrides.insert(target_index, override_value);
    }
    overrides
}

fn estimate_entry_tokens(entry: &Entry, override_value: Option<&Override>) -> u64 {
    match override_value {
        Some(Override::Omit) => 0,
        Some(Override::Replace(value)) => {
            chars_to_tokens(estimate_content_chars(content_of(value)))
        }
        None => match entry.kind.as_str() {
            "message" => entry
                .message
                .as_ref()
                .and_then(|message| message.content.as_ref())
                .map(estimate_content_chars)
                .map(chars_to_tokens)
                .unwrap_or(0),
            "custom_message" => entry
                .content
                .as_ref()
                .map(estimate_content_chars)
                .map(chars_to_tokens)
                .unwrap_or(0),
            "branch_summary" | "compaction" => entry
                .summary
                .as_deref()
                .map(|summary| chars_to_tokens(summary.chars().count()))
                .unwrap_or(0),
            _ => 0,
        },
    }
}

/// A `context_edit` replacement is `{ content: ... }`; plain content is also
/// tolerated, since the normalized shape is not enforced by the writer.
fn content_of(value: &serde_json::Value) -> &serde_json::Value {
    value.get("content").unwrap_or(value)
}

fn estimate_content_chars(content: &serde_json::Value) -> usize {
    match content {
        serde_json::Value::String(text) => text.chars().count(),
        serde_json::Value::Array(blocks) => blocks.iter().map(block_chars).sum(),
        _ => 0,
    }
}

fn block_chars(block: &serde_json::Value) -> usize {
    match block.get("type").and_then(|kind| kind.as_str()) {
        Some("text") => str_chars(block, "text"),
        Some("thinking") => str_chars(block, "thinking"),
        Some("toolCall") => {
            str_chars(block, "name")
                + block
                    .get("arguments")
                    .map(|args| args.to_string().chars().count())
                    .unwrap_or(0)
        }
        Some("image") => ESTIMATED_IMAGE_CHARS,
        _ => 0,
    }
}

fn str_chars(value: &serde_json::Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.chars().count())
        .unwrap_or(0)
}

/// pi's `Math.ceil(chars / 4)`.
fn chars_to_tokens(chars: usize) -> u64 {
    (chars as u64).div_ceil(4)
}

#[cfg(test)]
mod tests;
