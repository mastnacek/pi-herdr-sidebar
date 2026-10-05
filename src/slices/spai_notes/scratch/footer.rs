//! Scratchpad footer (plan §1, §2): mode, keys, current-line type hint,
//! live `→ target project` routing preview, save summary, Ctrl+S warning.
//! In Read mode: filters (`/` `~` `f`) and scope (Tab/Shift+Tab).
use super::state::{ScratchInput, ScratchMode, ScratchScope};
use super::super::state::SpaiNotesState;
use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn render_footer(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    let scratch = &state.scratch;

    let mut lines: Vec<Line> = Vec::new();

    // Footer input line (filter / semantic query) takes the first row.
    match scratch.input_mode {
        Some(ScratchInput::FuzzyFilter) => {
            lines.push(Line::from(vec![
                Span::styled(" / ", Style::default().fg(Color::Yellow).bold()),
                Span::styled(
                    format!("{}█", scratch.input_buffer),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    "  Enter = filtrovat · Esc = zrušit",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        Some(ScratchInput::SemanticFilter) => {
            lines.push(Line::from(vec![
                Span::styled(" ~ ", Style::default().fg(Color::Magenta).bold()),
                Span::styled(
                    format!("{}█", scratch.input_buffer),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    "  Enter = sémantické hledání (1 embedding + kosiny) · Esc = zrušit",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        None => {
            if scratch.mode == ScratchMode::Edit {
                lines.push(edit_hint_line(state));
            } else {
                lines.push(read_hint_line(state));
            }
        }
    }

    // Second row: mode + summary/warning.
    let mut second = Vec::new();
    second.push(Span::styled(
        format!(" -- {} -- ", if scratch.mode == ScratchMode::Edit { "EDIT" } else { "READ" }),
        Style::default()
            .fg(if scratch.mode == ScratchMode::Edit {
                Color::Green
            } else {
                Color::Cyan
            })
            .bold(),
    ));
    second.push(Span::styled(
        format!(" {} ", scratch.scope.label()),
        Style::default().fg(Color::DarkGray),
    ));
    if let Some(f) = &scratch.filter {
        let mut tokens = String::new();
        if let Some(t) = &f.text {
            tokens.push_str(&format!("/{} ", t));
        }
        if let Some(p) = &f.project {
            tokens.push_str(&format!("@{} ", p));
        }
        if let Some(t) = &f.tag {
            tokens.push_str(&format!(":{}: ", t));
        }
        if let Some(p) = &f.priority {
            tokens.push_str(&format!("!{} ", p));
        }
        second.push(Span::styled(
            format!("filtr: {}", tokens),
            Style::default().fg(Color::Yellow),
        ));
    }
    second.push(Span::styled(
        format!("· stav: {}", scratch.status_filter.label()),
        Style::default().fg(Color::DarkGray),
    ));
    if let Some(w) = &scratch.warn_similar {
        second.push(Span::styled(
            format!("  {}", w),
            Style::default().fg(Color::Rgb(255, 184, 108)).bold(),
        ));
    }
    if let Some(s) = &scratch.last_summary {
        second.push(Span::styled(
            format!("  {}", s),
            Style::default().fg(Color::Green),
        ));
    }
    if scratch.confirm_close {
        second.push(Span::styled(
            "  Neuložené řádky! Esc znovu = zavřít (draft zůstává)",
            Style::default().fg(Color::Red).bold(),
        ));
    }

    lines.push(Line::from(second));
    frame.render_widget(Paragraph::new(lines), area);
}

/// Edit mode hint: type of the current line only + live routing preview.
fn edit_hint_line(state: &SpaiNotesState) -> Line<'static> {
    let (type_hint, target) = state.scratch_footer_hint();
    Line::from(vec![
        Span::styled(
            format!(" {} ", type_hint),
            Style::default().fg(Color::White).bold(),
        ),
        Span::styled(
            format!("{}  ", target),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled("[Ctrl+S] ", Style::default().fg(Color::Green).bold()),
        Span::styled("uložit  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Ctrl+D] ", Style::default().fg(Color::Rgb(255, 184, 108)).bold()),
        Span::styled("duplicity  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Esc] ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("read", Style::default().fg(Color::DarkGray)),
    ])
}

fn read_hint_line(state: &SpaiNotesState) -> Line<'static> {
    let mut spans = vec![
        Span::styled("[↑/↓/j/k] ", Style::default().fg(Color::Yellow).bold()),
        Span::styled("záznam  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Tab] ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("scope  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Enter/o] ", Style::default().fg(Color::Green).bold()),
        Span::styled("otevřít  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[x/s] ", Style::default().fg(Color::Green).bold()),
        Span::styled("stav  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[/] ", Style::default().fg(Color::Yellow).bold()),
        Span::styled("filtr  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[~] ", Style::default().fg(Color::Magenta).bold()),
        Span::styled("sémantika  ", Style::default().fg(Color::DarkGray)),
    ];
    if !state.scratch.last_batch.is_empty() {
        spans.push(Span::styled("[u] ", Style::default().fg(Color::Red).bold()));
        spans.push(Span::styled(
            format!("vrátit dávku ({})  ", state.scratch.last_batch.len()),
            Style::default().fg(Color::DarkGray),
        ));
    }
    spans.push(Span::styled("[i] ", Style::default().fg(Color::Cyan).bold()));
    spans.push(Span::styled("edit  ", Style::default().fg(Color::DarkGray)));
    spans.push(Span::styled("[q] ", Style::default().fg(Color::Cyan).bold()));
    spans.push(Span::styled("zavřít", Style::default().fg(Color::DarkGray)));
    Line::from(spans)
}

/// Scope list helper for the footer (New → Project → All).
pub fn scope_label(scope: ScratchScope) -> &'static str {
    scope.label()
}