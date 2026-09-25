//! SPAI live input syntax detection and highlighting (inspired by tui/src/spai).
use super::note::{SpaiStatus, SpaiType};
use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectedSpaiInput {
    pub kind: SpaiType,
    pub status: SpaiStatus,
    pub prefix_label: &'static str,
    pub prefix_glyph: &'static str,
    pub badge_color: Color,
}

impl Default for DetectedSpaiInput {
    fn default() -> Self {
        Self {
            kind: SpaiType::Todo,
            status: SpaiStatus::Todo,
            prefix_label: "Úkol",
            prefix_glyph: ". ",
            badge_color: Color::Rgb(241, 252, 121), // yellow
        }
    }
}

/// Detects SPAI record type, status, and theme color from current input line.
pub fn detect_spai_input(input: &str) -> DetectedSpaiInput {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return DetectedSpaiInput::default();
    }

    if trimmed.starts_with("/. ") || trimmed.starts_with("/· ") {
        DetectedSpaiInput {
            kind: SpaiType::Todo,
            status: SpaiStatus::Waiting,
            prefix_label: "Úkol (čekající)",
            prefix_glyph: "/. ",
            badge_color: Color::Rgb(189, 147, 249), // violet
        }
    } else if trimmed.starts_with(". ") {
        DetectedSpaiInput {
            kind: SpaiType::Todo,
            status: SpaiStatus::Todo,
            prefix_label: "Úkol (k řešení)",
            prefix_glyph: ". ",
            badge_color: Color::Rgb(241, 252, 121), // yellow
        }
    } else if trimmed.starts_with("/ ") {
        DetectedSpaiInput {
            kind: SpaiType::Todo,
            status: SpaiStatus::Working,
            prefix_label: "Úkol (rozpracovaný)",
            prefix_glyph: "/ ",
            badge_color: Color::Rgb(139, 233, 253), // cyan
        }
    } else if trimmed.starts_with("x ") || trimmed.starts_with("X ") {
        DetectedSpaiInput {
            kind: SpaiType::Todo,
            status: SpaiStatus::Done,
            prefix_label: "Úkol (hotovo)",
            prefix_glyph: "x ",
            badge_color: Color::Rgb(55, 244, 153), // mint
        }
    } else if trimmed.starts_with("z ") || trimmed.starts_with("Z ") {
        DetectedSpaiInput {
            kind: SpaiType::Todo,
            status: SpaiStatus::Cancelled,
            prefix_label: "Úkol (zrušeno)",
            prefix_glyph: "z ",
            badge_color: Color::DarkGray,
        }
    } else if trimmed.starts_with("? ") {
        DetectedSpaiInput {
            kind: SpaiType::Idea,
            status: SpaiStatus::Idea,
            prefix_label: "Nápad / Inbox",
            prefix_glyph: "? ",
            badge_color: Color::Rgb(255, 121, 198), // pink
        }
    } else if trimmed.starts_with("- ") {
        DetectedSpaiInput {
            kind: SpaiType::Note,
            status: SpaiStatus::Note,
            prefix_label: "Poznámka",
            prefix_glyph: "- ",
            badge_color: Color::Rgb(139, 233, 253), // light cyan
        }
    } else if trimmed.starts_with("!- ") {
        DetectedSpaiInput {
            kind: SpaiType::Note,
            status: SpaiStatus::Note,
            prefix_label: "Kritická poznámka",
            prefix_glyph: "!- ",
            badge_color: Color::LightRed,
        }
    } else {
        // Fallback: default to Todo if no recognized prefix yet
        DetectedSpaiInput::default()
    }
}

/// Tokenizes input into styled spans with live SPAI syntax coloring.
pub fn highlight_spai_input_spans(input: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if input.is_empty() {
        return spans;
    }

    let detected = detect_spai_input(input);
    let mut rest = input;

    // 1. Highlight matching prefix if present
    for p in &[
        "/. ", "/· ", "!- ", ". ", "/ ", "x ", "X ", "z ", "Z ", "? ", "- ",
    ] {
        if rest.starts_with(p) {
            spans.push(Span::styled(
                p.to_string(),
                Style::default()
                    .fg(detected.badge_color)
                    .add_modifier(Modifier::BOLD),
            ));
            rest = &rest[p.len()..];
            break;
        }
    }

    // 2. Tokenize the remaining words (priority !, tags :tag:, project @proj, date @YYYY-MM-DD)
    let mut word_start = 0;
    let chars: Vec<(usize, char)> = rest.char_indices().collect();

    let mut i = 0;
    while i < chars.len() {
        let (idx, c) = chars[i];
        if c.is_whitespace() {
            if idx > word_start {
                let word = &rest[word_start..idx];
                spans.push(style_spai_word(word));
            }
            spans.push(Span::raw(" "));
            word_start = idx + c.len_utf8();
        }
        i += 1;
    }

    if word_start < rest.len() {
        let word = &rest[word_start..];
        spans.push(style_spai_word(word));
    }

    spans
}

fn style_spai_word(word: &str) -> Span<'static> {
    if word == "!" || word == "!!" || word == "!!!" {
        // Priority marker
        Span::styled(
            word.to_string(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else if word.starts_with('@') && word.len() > 1 {
        // Project or date facet
        Span::styled(
            word.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else if word.starts_with(':') && word.ends_with(':') && word.len() > 2 {
        // Tag :work:
        Span::styled(
            word.to_string(),
            Style::default().fg(Color::Rgb(55, 244, 153)), // mint
        )
    } else if word.starts_with('#') {
        // Hash tag
        Span::styled(word.to_string(), Style::default().fg(Color::Magenta))
    } else {
        Span::styled(word.to_string(), Style::default().fg(Color::White))
    }
}
