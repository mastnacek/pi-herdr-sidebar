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
    pub short_label: &'static str,
    pub prefix_glyph: &'static str,
    pub badge_color: Color,
}

impl Default for DetectedSpaiInput {
    fn default() -> Self {
        Self {
            kind: SpaiType::Todo,
            status: SpaiStatus::Todo,
            prefix_label: "Úkol",
            short_label: "Úkol",
            prefix_glyph: ". ",
            badge_color: Color::Rgb(241, 252, 121), // yellow
        }
    }
}

type PrefixSpec = (&'static [&'static str], SpaiType, SpaiStatus, &'static str, &'static str, &'static str, Color);

const PREFIX_SPECS: &[PrefixSpec] = &[
    (&["/. ", "/· ", "/.", "/·"], SpaiType::Todo, SpaiStatus::Waiting, "Čeká", "Čeká", "/. ", Color::Rgb(189, 147, 249)),
    (&[". ", "."], SpaiType::Todo, SpaiStatus::Todo, "Úkol", "Úkol", ". ", Color::Rgb(241, 252, 121)),
    (&["/ ", "/"], SpaiType::Todo, SpaiStatus::Working, "Rozpracováno", "Rozpracováno", "/ ", Color::Rgb(241, 252, 121)),
    (&["x ", "X ", "x", "X"], SpaiType::Todo, SpaiStatus::Done, "Hotovo", "Hotovo", "x ", Color::Rgb(55, 244, 153)),
    (&["z ", "Z ", "z", "Z"], SpaiType::Todo, SpaiStatus::Cancelled, "Zrušeno", "Zrušeno", "z ", Color::Rgb(135, 145, 170)),
    (&["? ", "?"], SpaiType::Idea, SpaiStatus::Idea, "Nápad", "Nápad", "? ", Color::Rgb(255, 121, 198)),
    (&["!- ", "!-"], SpaiType::Note, SpaiStatus::Note, "Kritická poznámka", "Kritická", "!- ", Color::Rgb(255, 83, 69)),
    (&["- ", "-"], SpaiType::Note, SpaiStatus::Note, "Poznámka", "Poznámka", "- ", Color::Rgb(139, 233, 253)),
    (&["* ", "*"], SpaiType::Note, SpaiStatus::Win, "Výhra", "Výhra", "* ", Color::Rgb(163, 230, 53)),
    (&["% ", "%"], SpaiType::Note, SpaiStatus::Fuckup, "Průser", "Průser", "% ", Color::Rgb(214, 69, 69)),
    (&["; ", ";"], SpaiType::Note, SpaiStatus::Skutek, "Skutek dne", "Skutek", "; ", Color::Rgb(255, 94, 219)),
];

/// Detects SPAI record type, status, and theme color from current input line.
pub fn detect_spai_input(input: &str) -> DetectedSpaiInput {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return DetectedSpaiInput::default();
    }

    let mut rest = trimmed;
    if let Some((first, remainder)) = rest.split_once(' ') {
        if is_time_format(first) {
            rest = remainder.trim_start();
        }
    }
    if let Some((first, remainder)) = rest.split_once(' ') {
        if first == "!" || first == "!!" || first == "!!!" {
            rest = remainder.trim_start();
        }
    } else if rest == "!" || rest == "!!" || rest == "!!!" {
        return DetectedSpaiInput::default();
    }

    for (prefixes, kind, status, label, short, glyph, color) in PREFIX_SPECS {
        for p in *prefixes {
            if (p.ends_with(' ') && rest.starts_with(p)) || rest == *p {
                return DetectedSpaiInput {
                    kind: *kind,
                    status: *status,
                    prefix_label: label,
                    short_label: short,
                    prefix_glyph: glyph,
                    badge_color: *color,
                };
            }
        }
    }

    DetectedSpaiInput::default()
}

/// Tokenizes input into styled spans with live SPAI syntax coloring.
pub fn highlight_spai_input_spans(input: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if input.is_empty() {
        return spans;
    }

    let mut rest = input;

    for (prefixes, _, _, _, _, _, color) in PREFIX_SPECS {
        for p in *prefixes {
            if rest.starts_with(p) {
                spans.push(Span::styled(
                    p.to_string(),
                    Style::default().fg(*color).add_modifier(Modifier::BOLD),
                ));
                rest = &rest[p.len()..];
                break;
            }
        }
        if rest.len() < input.len() {
            break;
        }
    }

    let mut word_start = 0;
    let chars: Vec<(usize, char)> = rest.char_indices().collect();

    for (idx, c) in &chars {
        if c.is_whitespace() {
            if *idx > word_start {
                colorize_spai_word(&rest[word_start..*idx], &mut spans);
            }
            spans.push(Span::raw(rest[*idx..*idx + c.len_utf8()].to_string()));
            word_start = *idx + c.len_utf8();
        }
    }

    if word_start < rest.len() {
        colorize_spai_word(&rest[word_start..], &mut spans);
    }

    spans
}

fn colorize_spai_word(word: &str, spans: &mut Vec<Span<'static>>) {
    if is_time_format(word) {
        spans.push(Span::styled(
            word.to_string(),
            Style::default().fg(Color::Rgb(241, 252, 121)),
        ));
    } else if let Some((color, bold)) = is_priority_word(word) {
        let mut style = Style::default().fg(color);
        if bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        spans.push(Span::styled(word.to_string(), style));
    } else if word.starts_with('@') && word.len() > 1 {
        spans.push(Span::styled(
            word.to_string(),
            Style::default()
                .fg(Color::Rgb(45, 213, 183))
                .add_modifier(Modifier::BOLD),
        ));
    } else if word.starts_with(':') && word.len() > 1 {
        colorize_tag_spans(word, spans);
    } else {
        spans.push(Span::styled(
            word.to_string(),
            Style::default().fg(Color::Rgb(245, 245, 245)),
        ));
    }
}

fn colorize_tag_spans(word: &str, spans: &mut Vec<Span<'static>>) {
    let mut rest = word;
    while !rest.is_empty() {
        if rest.starts_with(':') {
            if let Some(end) = rest[1..].find(':') {
                let tag_with_colons = &rest[..end + 2];
                let tag_inner = &rest[1..end + 1];
                spans.push(Span::styled(
                    tag_with_colons.to_string(),
                    Style::default().fg(classify_tag_color(tag_inner)),
                ));
                rest = &rest[end + 2..];
            } else {
                spans.push(Span::styled(
                    rest.to_string(),
                    Style::default().fg(classify_tag_color(&rest[1..])),
                ));
                break;
            }
        } else {
            spans.push(Span::styled(
                rest.to_string(),
                Style::default().fg(Color::Rgb(55, 244, 153)),
            ));
            break;
        }
    }
}

fn classify_tag_color(tag_inner: &str) -> Color {
    if let Some(pos) = tag_inner.find(['-', '+']) {
        if pos > 0 {
            let after_sign = &tag_inner[pos + 1..];
            if after_sign.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return if tag_inner.as_bytes()[pos] == b'-' {
                    Color::Rgb(255, 83, 69) // red expense
                } else {
                    Color::Rgb(55, 244, 153) // green income
                };
            }
        }
    }
    Color::Rgb(55, 244, 153) // mint default
}

fn is_priority_word(word: &str) -> Option<(Color, bool)> {
    let lower = word.to_lowercase();
    match lower.as_str() {
        "!" | "!!" | "!!!" | "!high" | "!vysoka" | "!vysoká" => {
            Some((Color::Rgb(255, 83, 69), true))
        }
        "!medium" | "!stredni" | "!střední" => {
            Some((Color::Rgb(241, 252, 121), true))
        }
        "!low" | "!nizka" | "!nízká" => {
            Some((Color::Rgb(139, 233, 253), false))
        }
        _ if word.starts_with('!') && word.len() > 1 => {
            Some((Color::Rgb(255, 83, 69), true))
        }
        _ => None,
    }
}

pub fn is_time_format(word: &str) -> bool {
    if word.contains(':') {
        let parts: Vec<&str> = word.split(':').collect();
        if parts.len() == 2 {
            let hours_ok = parts[0].parse::<u8>().map(|h| h < 24).unwrap_or(false);
            let mins_ok = parts[1].parse::<u8>().map(|m| m < 60).unwrap_or(false);
            return hours_ok && mins_ok;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_all_spai_prefixes() {
        assert_eq!(detect_spai_input(". task").short_label, "Úkol");
        assert_eq!(detect_spai_input("/ working").short_label, "Rozpracováno");
        assert_eq!(detect_spai_input("/. waiting").short_label, "Čeká");
        assert_eq!(detect_spai_input("x done").short_label, "Hotovo");
        assert_eq!(detect_spai_input("z cancelled").short_label, "Zrušeno");
        assert_eq!(detect_spai_input("? idea").short_label, "Nápad");
        assert_eq!(detect_spai_input("- note").short_label, "Poznámka");
        assert_eq!(detect_spai_input("!- critical").short_label, "Kritická");
    }

    #[test]
    fn highlights_syntax_tokens() {
        let spans = highlight_spai_input_spans(". @proj !high :tag: text");
        assert!(!spans.is_empty());
        assert_eq!(spans[0].content, ". ");
    }
}
