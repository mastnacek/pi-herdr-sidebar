//! Line builders for the simplified SPAI Smart Input dialog.
//!
//! Two hint surfaces only (plan §1):
//! - [`build_type_overview`] — dim, shown inside the editor while the input is
//!   empty: the complete type list plus the shared syntax cheat sheet.
//! - [`build_type_footer`] — 1–2 lines under the editor once a type is chosen,
//!   describing just that type with an example.
use crate::slices::spai_notes::autocomplete::ProjectSuggestion;
use crate::slices::spai_notes::type_options::{SpaiTypeOption, SPAI_TYPE_OPTIONS};
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span},
};

/// Dim overview for the empty input: every type with its symbol, then the
/// `@projekt !priorita :tagy: @termín` syntax line.
pub fn build_type_overview() -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        "  Typy — symbol + mezera na začátek textu:",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::raw(""));

    for opt in SPAI_TYPE_OPTIONS {
        lines.push(Line::from(vec![
            Span::styled(
                format!("    {:<3}", opt.display_sym),
                Style::default().fg(Color::DarkGray).bold(),
            ),
            Span::styled(
                format!(" {}", opt.name),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("  Syntax: ", Style::default().fg(Color::DarkGray).bold()),
        Span::styled("@projekt ", Style::default().fg(Color::DarkGray).bold()),
        Span::styled("!priorita ", Style::default().fg(Color::DarkGray).bold()),
        Span::styled(":tagy: ", Style::default().fg(Color::DarkGray).bold()),
        Span::styled("@termín", Style::default().fg(Color::DarkGray).bold()),
    ]));

    lines
}

/// Contextual footer for the selected type: name, description, first example.
pub fn build_type_footer(opt: &SpaiTypeOption) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {} ", opt.display_sym),
            Style::default().fg(opt.color).bold(),
        ),
        Span::styled(
            format!("{}: ", opt.name),
            Style::default().fg(Color::White).bold(),
        ),
        Span::styled(
            format!("{} · př.: {}", opt.desc, opt.examples.first().unwrap_or(&"")),
            Style::default().fg(Color::DarkGray),
        ),
    ])
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
        Span::styled("typ   ", Style::default().fg(Color::White)),
        Span::styled("[↑/↓] ", Style::default().fg(Color::DarkGray)),
        Span::styled("typ (prázdný vstup)   ", Style::default().fg(Color::White)),
        Span::styled("[Ctrl+D] ", Style::default().fg(Color::DarkGray)),
        Span::styled("duplicity   ", Style::default().fg(Color::White)),
        Span::styled("[Enter] ", Style::default().fg(Color::DarkGray)),
        Span::styled("uložit   ", Style::default().fg(Color::White)),
        Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
        Span::styled("zavřít", Style::default().fg(Color::White)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_lists_every_type_dimmed() {
        let lines = build_type_overview();
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        for opt in SPAI_TYPE_OPTIONS {
            assert!(text.contains(opt.name), "missing {}: {text}", opt.name);
        }
        assert!(text.contains("@projekt"), "syntax line: {text}");
        assert!(text.contains("!priorita"), "syntax line: {text}");
    }

    #[test]
    fn footer_describes_only_the_selected_type() {
        let todo = &SPAI_TYPE_OPTIONS[0];
        let line = build_type_footer(todo);
        let text = line.to_string();
        assert!(text.contains(todo.desc), "desc: {text}");
        assert!(text.contains("př.:"), "example: {text}");
        // No other type's description leaks in.
        let idea = &SPAI_TYPE_OPTIONS[5];
        assert!(!text.contains(idea.desc), "footer must be single-type: {text}");
    }
}
