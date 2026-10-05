//! Sub-components and lines builders for SPAI Smart Input creation dialog.
use crate::slices::spai_notes::autocomplete::ProjectSuggestion;
use crate::slices::spai_notes::note::SpaiNoteItem;
use crate::slices::spai_notes::similarity::{find_similar_notes, SimilarNoteMatch};
use crate::slices::spai_notes::type_options::SpaiTypeOption;
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span},
};
use std::time::Instant;

pub fn build_hint_lines(
    sel_opt: &SpaiTypeOption,
    raw_input: &str,
    existing_items: &[SpaiNoteItem],
    debounced_matches: &[SimilarNoteMatch],
    is_evaluating_vector: bool,
    last_keystroke: Option<Instant>,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let is_empty = raw_input.trim().is_empty();

    // 1. Live Deduplication / Warning Card (if duplicates detected or evaluating)
    if is_evaluating_vector {
        lines.push(Line::from(vec![
            Span::styled("  ⠋ ", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                "Vektorizuji zápis a porovnávám embeddingy přes OpenRouter...",
                Style::default().fg(Color::Yellow).bold(),
            ),
        ]));
        lines.push(Line::raw(""));
    } else if !debounced_matches.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  ⚠️  ", Style::default().fg(Color::Rgb(255, 184, 108)).bold()),
            Span::styled(
                "Nalezeny podobné existující záznamy (sémantický dedup):",
                Style::default().fg(Color::Rgb(255, 184, 108)).bold(),
            ),
        ]));

        for m in debounced_matches {
            let pct = (m.similarity * 100.0).round() as u32;
            let tag_label = if m.is_vector_match { "[Vektor] " } else { "" };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("    [{:>2}%] {}", pct, tag_label),
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
            Span::styled("    Akce: ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Ctrl+O] ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
            Span::styled("Otevřít   ", Style::default().fg(Color::Gray)),
            Span::styled("[Ctrl+A] ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
            Span::styled("Připojit k němu   ", Style::default().fg(Color::Gray)),
            Span::styled("[Ctrl+U] ", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
            Span::styled("Změnit stav", Style::default().fg(Color::Gray)),
        ]));
        lines.push(Line::raw(""));
    } else if let Some(last) = last_keystroke {
        if raw_input.trim().len() >= 3 && last.elapsed().as_millis() < 2200 {
            let left_secs = (2200u64.saturating_sub(last.elapsed().as_millis() as u64) as f64) / 1000.0;
            lines.push(Line::from(vec![Span::styled(
                format!("  ⏳ Dokončete psaní (vektorová kontrola za {:.1}s)...", left_secs),
                Style::default().fg(Color::DarkGray).italic(),
            )]));
            lines.push(Line::raw(""));
        }
    } else if !is_empty {
        // Fast local fallback when typing
        let similar = find_similar_notes(raw_input, existing_items, 0.45, 2);
        if !similar.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  ⚠️  ", Style::default().fg(Color::Rgb(255, 184, 108)).bold()),
                Span::styled(
                    "Podobné existující záznamy (textová shoda):",
                    Style::default().fg(Color::Rgb(255, 184, 108)).bold(),
                ),
            ]));
            for m in &similar {
                let pct = (m.similarity * 100.0).round() as u32;
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("    [{:>2}%] ", pct),
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
    }

    // 2. Helper text and Examples: Only show when the input is empty!
    if is_empty {
        lines.push(Line::from(vec![
            Span::styled("  Popis: ", Style::default().fg(Color::DarkGray)),
            Span::styled(sel_opt.desc, Style::default().fg(Color::White)),
        ]));
        lines.push(Line::raw(""));

        lines.push(Line::from(vec![
            Span::styled("  Příklady zápisu:", Style::default().fg(Color::Rgb(45, 213, 183)).bold()),
        ]));
        for ex in sel_opt.examples {
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(*ex, Style::default().fg(Color::White)),
            ]));
        }
        lines.push(Line::raw(""));
    }

    // 3. Cheat sheet (always accessible at bottom)
    lines.push(Line::from(vec![
        Span::styled("  SPAI Syntax:", Style::default().fg(Color::Rgb(139, 233, 253)).bold()),
        Span::styled("  @projekt ", Style::default().fg(Color::Cyan).bold()),
        Span::styled(" !priorita ", Style::default().fg(Color::LightRed).bold()),
        Span::styled(" :tagy: ", Style::default().fg(Color::Yellow).bold()),
        Span::styled(" @termín", Style::default().fg(Color::Magenta).bold()),
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
        Span::styled("Vložit prefix   ", Style::default().fg(Color::White)),
        Span::styled("[↑/↓] ", Style::default().fg(Color::DarkGray)),
        Span::styled("Typ záznamu   ", Style::default().fg(Color::White)),
        Span::styled("[Enter] ", Style::default().fg(Color::DarkGray)),
        Span::styled("Uložit   ", Style::default().fg(Color::White)),
        Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
        Span::styled("Zavřít", Style::default().fg(Color::White)),
    ])
}
