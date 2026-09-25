//! Deterministic word-wrapping for the SPAI note editor body.
//!
//! The editor has to know *which display row the cursor sits on* so it can keep
//! the cursor inside the visible viewport. Ratatui can only report that through
//! the unstable `Paragraph::line_count` API, so the body is pre-wrapped here and
//! rendered without `Wrap` instead. That keeps row bookkeeping exact, keeps the
//! dependency set stable, and makes the layout unit-testable.

/// Approximate display width of a char in terminal cells.
///
/// A deliberately small heuristic: wide CJK/emoji ranges count as two cells,
/// everything else as one. Notes are mostly Latin prose, so this is precise
/// there and merely conservative for exotic input.
fn char_width(c: char) -> usize {
    match c as u32 {
        0x1100..=0x115F
        | 0x2E80..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F
        | 0x1F900..=0x1F9FF
        | 0x1FA70..=0x1FAFF
        | 0x2600..=0x27BF => 2,
        _ => 1,
    }
}

pub fn text_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Splits a line into `(word, following_whitespace)` tokens.
fn tokenize(line: &str) -> Vec<(String, String)> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut spaces = String::new();

    for c in line.chars() {
        if c.is_whitespace() {
            if !word.is_empty() {
                tokens.push((std::mem::take(&mut word), String::new()));
            }
            spaces.push(c);
        } else {
            if !spaces.is_empty() {
                if let Some(last) = tokens.last_mut() {
                    last.1 = std::mem::take(&mut spaces);
                } else {
                    // Line starts with whitespace: keep it as its own token.
                    tokens.push((String::new(), std::mem::take(&mut spaces)));
                }
            }
            word.push(c);
        }
    }

    if !word.is_empty() {
        tokens.push((word, String::new()));
    }
    if !spaces.is_empty() {
        if let Some(last) = tokens.last_mut() {
            last.1.push_str(&spaces);
        } else {
            tokens.push((String::new(), spaces));
        }
    }

    tokens
}

/// Splits `word` into chunks that each fit within `width` cells.
fn hard_chunks(word: &str, width: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;

    for c in word.chars() {
        let w = char_width(c);
        if cur_w + w > width && !cur.is_empty() {
            chunks.push(std::mem::take(&mut cur));
            cur_w = 0;
        }
        cur.push(c);
        cur_w += w;
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    chunks
}

/// Wraps `text` (which may contain `\n`) into display rows no wider than `width`.
///
/// Newlines reset the row; soft wraps do not drop characters, so
/// `wrap_rows(text, w).concat()` equals `text` with `\n` removed.
pub fn wrap_rows(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut rows: Vec<String> = Vec::new();

    for line in text.split('\n') {
        let mut cur = String::new();
        let mut cur_w = 0usize;

        for (word, spaces) in tokenize(line) {
            let word_w = text_width(&word);
            let spaces_w = text_width(&spaces);

            if !word.is_empty() {
                if cur_w > 0 && cur_w + word_w > width {
                    rows.push(std::mem::take(&mut cur));
                    cur_w = 0;
                }
                if word_w > width {
                    // Over-long token (URL, hash): break it across rows.
                    let mut chunks = hard_chunks(&word, width);
                    let tail = chunks.pop().unwrap_or_default();
                    for chunk in chunks {
                        let chunk_w = text_width(&chunk);
                        if cur_w > 0 {
                            rows.push(std::mem::take(&mut cur));
                        }
                        rows.push(chunk);
                        cur_w = 0;
                        debug_assert!(chunk_w <= width);
                    }
                    cur.push_str(&tail);
                    cur_w = text_width(&tail);
                } else {
                    cur.push_str(&word);
                    cur_w += word_w;
                }
            }

            if !spaces.is_empty() {
                if cur_w > 0 && cur_w + spaces_w > width {
                    rows.push(std::mem::take(&mut cur));
                    cur_w = 0;
                }
                cur.push_str(&spaces);
                cur_w += spaces_w;
            }
        }

        rows.push(cur);
    }

    if rows.is_empty() {
        rows.push(String::new());
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_is_a_single_row() {
        assert_eq!(wrap_rows("hello", 20), vec!["hello"]);
        assert_eq!(wrap_rows("", 20), vec![""]);
    }

    #[test]
    fn newlines_create_hard_rows() {
        assert_eq!(wrap_rows("a\nbb\nccc", 10), vec!["a", "bb", "ccc"]);
    }

    #[test]
    fn words_wrap_without_being_split() {
        let rows = wrap_rows("alpha beta gamma", 11);
        assert_eq!(rows, vec!["alpha beta ", "gamma"]);
        for row in &rows {
            assert!(text_width(row) <= 11, "row too wide: {row:?}");
        }
    }

    #[test]
    fn over_long_words_are_hard_broken() {
        let rows = wrap_rows("https://example.com/very/long/path", 10);
        assert!(rows.len() > 1, "must break: {rows:?}");
        for row in &rows {
            assert!(text_width(row) <= 10, "row too wide: {row:?}");
        }
    }

    #[test]
    fn no_characters_are_lost() {
        let text = "první řádek s diakritikou\nhttps://a.b/c?d=e&f=g druhý ✓ řádek";
        let rows = wrap_rows(text, 9);
        assert_eq!(rows.concat(), text.replace('\n', ""));
    }

    #[test]
    fn wide_chars_count_as_two_cells() {
        assert_eq!(text_width("日本"), 4);
        assert!(wrap_rows("日本語", 4).len() >= 2);
    }

    #[test]
    fn width_one_never_panics() {
        let rows = wrap_rows("abc def", 1);
        assert_eq!(rows.concat(), "abc def");
    }
}
