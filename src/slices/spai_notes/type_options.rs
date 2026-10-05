//! SPAI item types definitions, colors, and syntax examples for smart input.
use super::note::{SpaiStatus, SpaiType};
use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaiTypeOption {
    pub symbol: &'static str,
    pub display_sym: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub examples: &'static [&'static str],
    pub kind: SpaiType,
    pub status: SpaiStatus,
    pub color: Color,
}

pub const SPAI_TYPE_OPTIONS: &[SpaiTypeOption] = &[
    SpaiTypeOption {
        symbol: ". ",
        display_sym: ".",
        name: "Úkol (pending)",
        desc: "Nový úkol čekající na zpracování",
        examples: &[
            ". @projekt !high úkol k vyřešení",
            ". připravit podklady pro schůzku :dok:",
        ],
        kind: SpaiType::Todo,
        status: SpaiStatus::Todo,
        color: Color::Rgb(255, 215, 0), // Gold
    },
    SpaiTypeOption {
        symbol: "/ ",
        display_sym: "/",
        name: "Rozpracováno (in progress)",
        desc: "Úkol, na kterém se aktuálně pracuje",
        examples: &[
            "/ @projekt práce na implementaci",
            "/ ladění testů a refaktoring",
        ],
        kind: SpaiType::Todo,
        status: SpaiStatus::Working,
        color: Color::Rgb(0, 255, 255), // Cyan
    },
    SpaiTypeOption {
        symbol: "/. ",
        display_sym: "/.",
        name: "Čeká (waiting)",
        desc: "Úkol čekající na externí vstup nebo někoho/něco",
        examples: &[
            "/. @projekt čekám na review PR",
            "/. čekám na schválení rozpočtu",
        ],
        kind: SpaiType::Todo,
        status: SpaiStatus::Waiting,
        color: Color::Rgb(255, 107, 107), // Coral
    },
    SpaiTypeOption {
        symbol: "x ",
        display_sym: "x",
        name: "Hotovo (done)",
        desc: "Dokončený a uzavřený úkol",
        examples: &[
            "x @projekt hotovo :2h:",
            "x nasazeno na produkční server",
        ],
        kind: SpaiType::Todo,
        status: SpaiStatus::Done,
        color: Color::Rgb(144, 238, 144), // Light green
    },
    SpaiTypeOption {
        symbol: "z ",
        display_sym: "z",
        name: "Zrušeno (cancelled)",
        desc: "Zrušený nebo neplatný úkol",
        examples: &[
            "z @projekt už není potřeba",
            "z zrušeno po dohodě s týmem",
        ],
        kind: SpaiType::Todo,
        status: SpaiStatus::Cancelled,
        color: Color::Rgb(127, 140, 141), // Dim gray
    },
    SpaiTypeOption {
        symbol: "? ",
        display_sym: "?",
        name: "Nápad (idea)",
        desc: "Myšlenka, nápad k pozdějšímu zpracování",
        examples: &[
            "? @projekt nový nápad na funkci",
            "? prozkoumat novou knihovnu pro TUI",
        ],
        kind: SpaiType::Idea,
        status: SpaiStatus::Idea,
        color: Color::Rgb(186, 85, 211), // Violet
    },
    SpaiTypeOption {
        symbol: "- ",
        display_sym: "-",
        name: "Poznámka (note)",
        desc: "Běžná textová poznámka nebo zápisek",
        examples: &[
            "- @projekt zápis ze standupu",
            "- odkaz na specifikaci API",
        ],
        kind: SpaiType::Note,
        status: SpaiStatus::Note,
        color: Color::Rgb(127, 179, 255), // Slate blue
    },
    SpaiTypeOption {
        symbol: "* ",
        display_sym: "*",
        name: "Výhra (win)",
        desc: "Co se dnes povedlo, úspěch nebo milník",
        examples: &[
            "* @projekt úspěšně nasazena v1.0",
            "* vyřešen dlouhodobý memory leak",
        ],
        kind: SpaiType::Note,
        status: SpaiStatus::Win,
        color: Color::Rgb(163, 230, 53), // Lime
    },
    SpaiTypeOption {
        symbol: "% ",
        display_sym: "%",
        name: "Průser (fuckup)",
        desc: "Co se nepovedlo a co nás to naučilo",
        examples: &[
            "% @projekt výpadek prod DB po špatné migraci",
            "% zapomenutý rollback plán pro deploy",
        ],
        kind: SpaiType::Note,
        status: SpaiStatus::Fuckup,
        color: Color::Rgb(214, 69, 69), // Crimson
    },
    SpaiTypeOption {
        symbol: "; ",
        display_sym: ";",
        name: "Skutek dne (skutek)",
        desc: "Hlavní dnešní počin nebo klíčový skutek",
        examples: &[
            "; @projekt dokončen kompletní refaktoring API",
            "; schválena nová architektura systému",
        ],
        kind: SpaiType::Note,
        status: SpaiStatus::Skutek,
        color: Color::Rgb(255, 94, 219), // Magenta
    },
    SpaiTypeOption {
        symbol: "# ",
        display_sym: "#",
        name: "Inbox capture",
        desc: "Rychlý záchyt myšlenky do inboxu",
        examples: &[
            "# @projekt rychlý záchyt požadavku",
            "# zkontrolovat logy z produkce",
        ],
        kind: SpaiType::Idea,
        status: SpaiStatus::Inbox,
        color: Color::Rgb(78, 205, 196), // Teal
    },
    SpaiTypeOption {
        symbol: "!- ",
        display_sym: "!-",
        name: "Kritická událost",
        desc: "Důležitá událost nebo varování k zapamatování",
        examples: &[
            "!- @projekt výpadek API v produkci",
            "!- kritická chyba v platební bráně",
        ],
        kind: SpaiType::Note,
        status: SpaiStatus::Note,
        color: Color::Rgb(255, 83, 69), // Red
    },
];

pub fn find_type_option_index(input: &str) -> Option<usize> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let mut rest = trimmed;
    if let Some((first, remainder)) = rest.split_once(' ') {
        if crate::slices::spai_notes::input_highlighter::is_time_format(first) {
            rest = remainder.trim_start();
        }
    }
    if let Some((first, remainder)) = rest.split_once(' ') {
        if first == "!" || first == "!!" || first == "!!!" {
            rest = remainder.trim_start();
        }
    }

    for (idx, opt) in SPAI_TYPE_OPTIONS.iter().enumerate() {
        if rest.starts_with(opt.symbol)
            || rest == opt.display_sym
            || (opt.display_sym == "/." && (rest.starts_with("/· ") || rest == "/·" || rest.starts_with("/.") || rest == "/."))
            || (opt.display_sym == "x" && (rest.starts_with("X ") || rest == "X" || rest.starts_with("x ") || rest == "x"))
            || (opt.display_sym == "z" && (rest.starts_with("Z ") || rest == "Z" || rest.starts_with("z ") || rest == "z"))
        {
            return Some(idx);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_spai_type_options() {
        assert_eq!(find_type_option_index(". test"), Some(0));
        assert_eq!(find_type_option_index("/ test"), Some(1));
        assert_eq!(find_type_option_index("/. test"), Some(2));
        assert_eq!(find_type_option_index("/· test"), Some(2));
        assert_eq!(find_type_option_index("x test"), Some(3));
        assert_eq!(find_type_option_index("X test"), Some(3));
        assert_eq!(find_type_option_index("z test"), Some(4));
        assert_eq!(find_type_option_index("? test"), Some(5));
        assert_eq!(find_type_option_index("- test"), Some(6));
        assert_eq!(find_type_option_index("* test"), Some(7));
        assert_eq!(find_type_option_index("% test"), Some(8));
        assert_eq!(find_type_option_index("; test"), Some(9));
        assert_eq!(find_type_option_index("# test"), Some(10));
        assert_eq!(find_type_option_index("!- test"), Some(11));
        assert_eq!(find_type_option_index("plain text"), None);
    }
}
