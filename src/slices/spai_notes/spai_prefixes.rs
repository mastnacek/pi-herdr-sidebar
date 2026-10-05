//! Single shared table of SPAI line prefixes.
//!
//! Every entry is the full prefix **including its trailing space** — bare
//! symbols (`"x"`, `"h"`, `"-"`, …) are deliberately not listed, otherwise
//! ordinary words like "hello" or "xylofon" lose their first letter when a
//! caller strips what it believes to be a prefix.
//!
//! Order matters: multi-character prefixes must come before their
//! single-character substrings (`"/. "` before `"/ "`, `"!/. "` before `"!. "`).
pub const SPAI_PREFIXES: &[&str] = &[
    "/. ", "/· ", "!. ", "!/ ", "!/. ", "!x ", "!X ", "!z ", "!Z ", "!? ", "!- ", "!+ ", "!= ",
    "!* ", "!% ", "!~ ", "!$ ", "!♥ ", "!# ", ". ", "/ ", "x ", "X ", "z ", "Z ", "? ", "- ",
    "+ ", "= ", "* ", "% ", "~ ", "$ ", "♥ ", "h ", "# ",
];

/// Strips a leading SPAI prefix from user input and returns the rest.
///
/// Returns `Some(rest)` only when the input starts with a full table entry
/// (symbol followed by a space), so `"hello"` and `"xylofon"` pass through
/// untouched while `". hello"` yields `"hello"`.
pub fn strip_leading_prefix(input: &str) -> Option<&str> {
    for p in SPAI_PREFIXES {
        if let Some(rest) = input.strip_prefix(p) {
            return Some(rest);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_only_full_prefixes_with_space() {
        assert_eq!(strip_leading_prefix(". hello"), Some("hello"));
        assert_eq!(strip_leading_prefix("/. čekám"), Some("čekám"));
        assert_eq!(strip_leading_prefix("!. urgentní"), Some("urgentní"));
        assert_eq!(strip_leading_prefix("x hotovo"), Some("hotovo"));
        assert_eq!(strip_leading_prefix("h zada 4"), Some("zada 4"));
        assert_eq!(strip_leading_prefix("♥ @zdravi hlava"), Some("@zdravi hlava"));
    }

    #[test]
    fn never_trims_the_first_letter_of_plain_words() {
        assert_eq!(strip_leading_prefix("hello"), None);
        assert_eq!(strip_leading_prefix("xylofon"), None);
        assert_eq!(strip_leading_prefix("zrušení"), None);
        assert_eq!(strip_leading_prefix("-0.5% ztráta"), None);
        assert_eq!(strip_leading_prefix("+420peněz"), None);
    }
}
