//! Chrome helpers for sidebar UI: empty state and footer bar.
use crate::slices::view::state::SidebarState;
use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

pub fn render_empty_state(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let dir = crate::shared::pi_sidebar_snapshots_dir();
    let sessions_dir = crate::shared::dirs_home()
        .map(|h| h.join(".pi").join("agent").join("sessions"))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/.pi/agent/sessions".to_string());
    let text = vec![
        Line::from(Span::styled(
            "Čekám na telemetrii pi agenta…",
            Style::default().fg(Color::Yellow).bold(),
        )),
        Line::raw(""),
        Line::from("Sidebar čte telemetrii přímo z logů pi relací:"),
        Line::from(Span::styled(sessions_dir, Style::default().fg(Color::Cyan))),
        Line::raw(""),
        Line::from("1. Spusť pi v herdr: 'pi'"),
        Line::from("2. Proved' libovolný tah — telemetrie se streamuje automaticky."),
        Line::from("3. Pro pohled Status není potřeba '/sidebar on' (snapshot"),
        Line::from("   je potřeba jen pro pohled Skilly)."),
        Line::raw(""),
        Line::from(vec![
            Span::raw("Adresář snapshotů (legacy): "),
            Span::styled(dir.display().to_string(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("Cílový panel: "),
            Span::styled(
                state.target_pane_id.as_deref().unwrap_or("none"),
                Style::default().fg(Color::Magenta),
            ),
        ]),
    ];

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Telemetrie Offline ");

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

pub fn render_footer(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let footer_text = Line::from(vec![
        Span::styled(" [Tab/1/2/3] ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Pohled  "),
        Span::styled(" [↑/↓/j/k] ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Posun  "),
        Span::styled(" [w] ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Lokalita  "),
        Span::styled(" [c] ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Kopírovat předpověď  "),
        Span::styled(" [r] ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Obnovit  "),
        Span::styled(" [q] ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Konec  "),
        Span::styled(
            format!("(Posun: {})", state.scroll),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let paragraph = Paragraph::new(footer_text);
    frame.render_widget(paragraph, area);
}
