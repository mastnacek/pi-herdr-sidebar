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
    assert_eq!(detect_spai_input("+ skutek").short_label, "Skutek");
    assert_eq!(detect_spai_input("= mood").short_label, "Nálada");
    assert_eq!(detect_spai_input("* win").short_label, "Výhra");
    assert_eq!(detect_spai_input("% fuckup").short_label, "Průser");
}

#[test]
fn detects_priority_prefixed_to_any_element() {
    assert_eq!(detect_spai_input("!. task").short_label, "Úkol");
    assert_eq!(detect_spai_input("!. task").badge_color, Color::Rgb(255, 83, 69));

    assert_eq!(detect_spai_input("!- critical note").short_label, "Poznámka");
    assert_eq!(detect_spai_input("!- critical note").badge_color, Color::Rgb(255, 83, 69));

    assert_eq!(detect_spai_input("!? critical idea").short_label, "Nápad");
    assert_eq!(detect_spai_input("!? critical idea").badge_color, Color::Rgb(255, 83, 69));

    assert_eq!(detect_spai_input("!+ key deed").short_label, "Skutek");
    assert_eq!(detect_spai_input("!+ key deed").badge_color, Color::Rgb(255, 83, 69));

    assert_eq!(detect_spai_input("!* big win").short_label, "Výhra");
    assert_eq!(detect_spai_input("!% big incident").short_label, "Průser");
}

#[test]
fn highlights_syntax_tokens() {
    let spans = highlight_spai_input_spans(". @proj !high :tag: text");
    assert!(!spans.is_empty());
    assert_eq!(spans[0].content, ". ");
}

#[test]
fn highlights_priority_prefixed_spans() {
    let spans = highlight_spai_input_spans("!. @proj urgent task");
    assert!(!spans.is_empty());
    assert_eq!(spans[0].content, "!");
    assert_eq!(spans[1].content.trim(), ".");
}
