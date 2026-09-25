//! Pure char-index cursor arithmetic for the SPAI note editor.
//!
//! The editor keeps its cursor as a *char* index (not a byte offset) so that
//! Czech diacritics and emoji edit correctly. Everything here is free of UI
//! state, which keeps [`super::state_edit`] focused on dialog behaviour.

/// Number of chars in `s`.
pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Char index → byte offset, clamped to the end of the string.
pub fn byte_of(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Start (inclusive) and end (exclusive) char indices of the line holding `cursor`.
pub fn line_bounds(s: &str, cursor: usize) -> (usize, usize) {
    let chars: Vec<char> = s.chars().collect();
    let cursor = cursor.min(chars.len());

    let mut start = 0;
    for i in (0..cursor).rev() {
        if chars[i] == '\n' {
            start = i + 1;
            break;
        }
    }

    let mut end = chars.len();
    for (i, c) in chars.iter().enumerate().skip(cursor) {
        if *c == '\n' {
            end = i;
            break;
        }
    }

    (start, end)
}

/// Moves one line up/down while keeping the column, clamped to the line length.
pub fn move_line(s: &str, cursor: usize, down: bool) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let cursor = cursor.min(chars.len());
    let (start, end) = line_bounds(s, cursor);
    let col = cursor - start;

    if down {
        if end >= chars.len() {
            return cursor;
        }
        let next_start = end + 1;
        let next_end = chars[next_start..]
            .iter()
            .position(|c| *c == '\n')
            .map(|p| next_start + p)
            .unwrap_or(chars.len());
        next_start + col.min(next_end - next_start)
    } else {
        if start == 0 {
            return cursor;
        }
        let prev_end = start - 1;
        let mut prev_start = 0;
        for i in (0..prev_end).rev() {
            if chars[i] == '\n' {
                prev_start = i + 1;
                break;
            }
        }
        prev_start + col.min(prev_end - prev_start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_offsets_are_char_based() {
        let s = "příliš";
        assert_eq!(char_len(s), 6);
        assert_eq!(byte_of(s, 0), 0);
        assert_eq!(byte_of(s, 1), 1);
        assert_eq!(byte_of(s, 2), 3, "ř is two bytes");
        assert_eq!(byte_of(s, 99), s.len(), "clamped to the end");
    }

    #[test]
    fn line_bounds_cover_the_cursor_line() {
        let s = "alpha\nbeta\ngamma";
        assert_eq!(line_bounds(s, 0), (0, 5));
        assert_eq!(
            line_bounds(s, 5),
            (0, 5),
            "cursor on the newline stays on its line"
        );
        assert_eq!(line_bounds(s, 6), (6, 10));
        assert_eq!(line_bounds(s, 16), (11, 16));
    }

    #[test]
    fn move_line_keeps_column_and_clamps_to_shorter_lines() {
        let s = "short\nmuch longer line";
        assert_eq!(move_line(s, 5, true), 11, "col 5 on the next line");
        assert_eq!(move_line(s, 11, false), 5, "back up");
        assert_eq!(move_line(s, 16, false), 5, "clamped to the short line end");
        assert_eq!(move_line(s, 0, false), 0, "top line cannot move up");
        assert_eq!(move_line(s, 16, true), 16, "bottom line cannot move down");
    }
}
