//! Sub-components and lines builders for SPAI Smart Input creation dialog.
use crate::slices::spai_notes::autocomplete::ProjectSuggestion;
use crate::slices::spai_notes::note::SpaiNoteItem;
use crate::slices::spai_notes::similarity::{find_similar_notes, SimilarNoteMatch};
use crate::slices::spai_notes::type_options::SpaiTypeOption;
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span},
};

pub fn build_hint_lines(
    sel_opt: &SpaiTypeOption,
    raw_input: &str,
    existing_items: &[SpaiNoteItem],
    debounced_matches: &[SimilarNoteMatch],
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![Span::styled(
            format!(" {}", sel_opt.desc),
            Style::default().fg(Color::Rgb(220, 220, 220)),
        )]),
        Line::raw(""),
    ];

    // Live Semantic Deduplication / Similarity check (debounced vector + fast local)
    let similar = if !debounced_matches.is_empty() {
        debounced_matches.to_vec()
    } else {
        find_similar_notes(raw_input, existing_items, 0.45, 3)
    };

    if !similar.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            " ▌ ⚠️  Podobné existující záznamy (živý sémantický dedup):",
            Style::default().fg(Color::Rgb(255, 184, 108)).bold(),
        )]));
        for m in &similar {
            let pct = (m.similarity * 100.0).round() as u32;
            let tag_label = if m.is_vector_match { "[Vektor] " } else { "" };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("   [{:>2}%] {}", pct, tag_label),
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
        lines.push(Line::from(vec![
            Span::styled("   Akce: ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Ctrl+O] ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
            Span::styled("Otevřít existující  ", Style::default().fg(Color::Gray)),
            Span::styled("[Ctrl+A] ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
            Span::styled("Připojit k němu  ", Style::default().fg(Color::Gray)),
            Span::styled("[Ctrl+U] ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
            Span::styled("Změnit stav", Style::default().fg(Color::Gray)),
        ]));
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
        Span::styled("   @projekt", Style::default().fg(Color::Cyan).bold()),
        Span::styled("        Přiřazení k projektu (@herdr, @piprompt)", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("   ! nebo !high", Style::default().fg(Color::LightRed).bold()),
        Span::styled("    Priorita / Důležitost (!, !high, !low)", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("   :tag1:tag2:", Style::default().fg(Color::Yellow).bold()),
        Span::styled("     Kategorie a tagy (:auth:api:security:)", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("   @termín", Style::default().fg(Color::Magenta).bold()),
        Span::styled("         Deadline (@dnes, @zitra, @2026-10-01)", Style::default().fg(Color::DarkGray)),
    ]));

    lines
}

pub fn build_project_picker_lines(
    suggestions: &[ProjectSuggestion],
    selected_idx: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (i, s) in suggestions.iter().enumerate() {
        let is_sel = i == selected_idx;
        let prefix = if is_sel { "▶ " } else { "  " };
        lines.push(Line::from(vec![
            Span::styled(
                prefix,
                if is_sel {
                    Style::default().fg(Color::Yellow).bold()
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(
                s.insert_text.clone(),
                if is_sel {
                    Style::default().fg(Color::Yellow).bold()
                } else {
                    Style::default().fg(Color::Cyan)
                },
            ),
            Span::styled(
                format!(" ({})", s.name),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }
    lines
}

pub fn build_shortcuts_line() -> Line<'static> {
    Line::from(vec![
        Span::styled(" [Tab] ", Style::default().fg(Color::DarkGray)),
        Span::styled("Potvrdit   ", Style::default().fg(Color::White)),
        Span::styled("[↑/↓] ", Style::default().fg(Color::DarkGray)),
        Span::styled("Typ záznamu   ", Style::default().fg(Color::White)),
        Span::styled("[Enter] ", Style::default().fg(Color::DarkGray)),
        Span::styled("Uložit   ", Style::default().fg(Color::White)),
        Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
        Span::styled("Zavřít", Style::default().fg(Color::White)),
    ])
}
