//! Sub-components and lines builders for SPAI Smart Input creation dialog.
use crate::slices::spai_notes::autocomplete::ProjectSuggestion;
use crate::slices::spai_notes::note::SpaiNoteItem;
use crate::slices::spai_notes::similarity::find_similar_notes;
use crate::slices::spai_notes::type_options::SpaiTypeOption;
use ratatui::{
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
};

pub fn build_hint_lines(
    sel_opt: &SpaiTypeOption,
    raw_input: &str,
    existing_items: &[SpaiNoteItem],
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![Span::styled(
            format!(" {}", sel_opt.desc),
            Style::default().fg(Color::Rgb(220, 220, 220)),
        )]),
        Line::raw(""),
    ];

    // Live Semantic Deduplication / Similarity check
    let similar = find_similar_notes(raw_input, existing_items, 0.45, 3);
    if !similar.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            " ▌ ⚠️  Podobné existující záznamy (dedup):",
            Style::default().fg(Color::Rgb(255, 184, 108)).bold(),
        )]));
        for m in similar {
            let pct = (m.similarity * 100.0).round() as u32;
            lines.push(Line::from(vec![
                Span::styled(
                    format!("   [{:>2}%] ", pct),
                    Style::default()
                        .fg(if pct >= 70 {
                            Color::Rgb(255, 83, 69)
                        } else {
                            Color::Rgb(255, 184, 108)
                        })
                        .bold(),
                ),
                Span::styled(
                    format!("{} ", m.symbol),
                    Style::default().fg(Color::Yellow).bold(),
                ),
                Span::styled(
                    format!("{}: {}", m.id, m.title),
                    Style::default().fg(Color::White),
                ),
            ]));
        }
        lines.push(Line::raw(""));
    }

    lines.push(Line::from(vec![Span::styled(
        " ▌ Příklady zápisu:",
        Style::default().fg(Color::Rgb(45, 213, 183)).bold(),
    )]));

    for ex in sel_opt.examples {
        lines.push(Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(*ex, Style::default().fg(Color::White)),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(vec![Span::styled(
        " ▌ SPAI Syntax & Dekorátory:",
        Style::default().fg(Color::Rgb(139, 233, 253)).bold(),
    )]));
    lines.push(Line::from(vec![
        Span::styled(
            "   ! [značka]   ",
            Style::default().fg(Color::Rgb(255, 83, 69)).bold(),
        ),
        Span::styled(
            "Kritický záznam (např. !., !-, !?, !+, !=, !*)",
            Style::default().fg(Color::Rgb(200, 200, 200)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "   @projekt     ",
            Style::default().fg(Color::Rgb(45, 213, 183)).bold(),
        ),
        Span::styled(
            "Přiřazení k projektu (@pi-spai, @\"můj projekt\")",
            Style::default().fg(Color::Rgb(200, 200, 200)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "   !high        ",
            Style::default().fg(Color::Rgb(255, 83, 69)).bold(),
        ),
        Span::styled(
            "Priorita: !high · !medium · !low",
            Style::default().fg(Color::Rgb(200, 200, 200)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "   :tag:        ",
            Style::default().fg(Color::Rgb(55, 244, 153)).bold(),
        ),
        Span::styled(
            "Štítky (:dev:rust: nebo finance :jidlo-500N:)",
            Style::default().fg(Color::Rgb(200, 200, 200)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "   @15.09.      ",
            Style::default().fg(Color::Rgb(241, 252, 121)).bold(),
        ),
        Span::styled(
            "Termín / deadline (@15.09. nebo @2026-09-15)",
            Style::default().fg(Color::Rgb(200, 200, 200)),
        ),
    ]));

    lines
}

pub fn build_shortcuts_line() -> Line<'static> {
    Line::from(vec![
        Span::styled(
            "  [↑/↓]",
            Style::default().fg(Color::Rgb(241, 252, 121)).bold(),
        ),
        Span::styled(" Vybrat typ   ", Style::default().fg(Color::Rgb(193, 196, 151))),
        Span::styled(
            "[Tab]",
            Style::default().fg(Color::Rgb(45, 213, 183)).bold(),
        ),
        Span::styled(
            " Vložit prefix / @Projekt   ",
            Style::default().fg(Color::Rgb(193, 196, 151)),
        ),
        Span::styled(
            "[Enter]",
            Style::default().fg(Color::Rgb(55, 244, 153)).bold(),
        ),
        Span::styled(" Uložit   ", Style::default().fg(Color::Rgb(193, 196, 151))),
        Span::styled(
            "[Esc]",
            Style::default().fg(Color::Rgb(241, 252, 121)).bold(),
        ),
        Span::styled(" Zrušit", Style::default().fg(Color::Rgb(193, 196, 151))),
    ])
}

pub fn build_project_picker_lines(
    suggestions: &[ProjectSuggestion],
    selected: usize,
) -> Vec<Line<'static>> {
    let mut ac_lines = Vec::new();
    for (i, sug) in suggestions.iter().enumerate() {
        let is_sel = i == selected;
        let marker = if is_sel { "▶ " } else { "  " };
        ac_lines.push(Line::from(vec![
            Span::styled(
                marker,
                Style::default().fg(if is_sel {
                    Color::Rgb(241, 252, 121)
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                format!("{:<18}", sug.insert_text),
                Style::default()
                    .fg(if is_sel {
                        Color::Rgb(45, 213, 183)
                    } else {
                        Color::White
                    })
                    .add_modifier(if is_sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::styled(
                format!(" {}", sug.path),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }
    ac_lines
}
