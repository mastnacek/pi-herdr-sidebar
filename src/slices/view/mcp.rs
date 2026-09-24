//! MCP face — server usage + recent tool calls.
use super::ui::spinner_char;
use crate::slices::telemetry::fmt_tokens;
use crate::slices::view::state::SidebarState;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};
use ratatui::Frame;

/// MCP Servers & Calls inspection face.
pub fn render_mcp_face(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let spinner = spinner_char(state.anim_tick);

    let Some(mcp) = &state.mcp else {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Magenta))
            .title(" MCP Servery ");
        let text = vec![
            Line::raw(""),
            Line::from(Span::styled(
                "🔌 Žádná aktivita MCP serverů",
                Style::default().fg(Color::DarkGray),
            )),
            Line::raw(""),
            Line::from("Sidebar sleduje MCP volání (Knowledge Base, LSP, OpenRouter, atd.)"),
            Line::from("přímo ze zdrojového protokolu pi relace."),
        ];
        frame.render_widget(
            Paragraph::new(text)
                .block(block)
                .scroll((state.scroll, 0))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    };

    let status_badge = if mcp.in_flight {
        Span::styled(
            format!("{} V BĚHU", spinner),
            Style::default().fg(Color::Yellow).bold(),
        )
    } else {
        Span::styled("● KLID", Style::default().fg(Color::Green).bold())
    };

    let header_line = Line::from(vec![
        Span::styled(
            "🔌 MCP Aktivita: ",
            Style::default().fg(Color::Magenta).bold(),
        ),
        status_badge,
        Span::raw("  "),
        Span::styled(
            format!("{} volání", mcp.total_calls),
            Style::default().fg(Color::White),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("📦 ~{} tok", fmt_tokens(mcp.total_tokens)),
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        if mcp.total_errors > 0 {
            Span::styled(
                format!("⚠️ {} chyb", mcp.total_errors),
                Style::default().fg(Color::Red).bold(),
            )
        } else {
            Span::styled("✓ 0 chyb", Style::default().fg(Color::Green))
        },
    ]);

    let mut lines = vec![header_line, Line::raw("")];

    // Configured / active servers
    lines.push(Line::from(Span::styled(
        "📦 Použité servery:",
        Style::default().fg(Color::Cyan).bold(),
    )));
    if mcp.servers_used.is_empty() {
        lines.push(Line::from(Span::styled(
            "   (zatím žádný server nepoužit)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let mut server_spans = vec![Span::raw("   ")];
        for s in &mcp.servers_used {
            server_spans.push(Span::styled(
                format!("[{}] ", s),
                Style::default().fg(Color::Green).bold(),
            ));
        }
        lines.push(Line::from(server_spans));
    }
    lines.push(Line::raw(""));

    // Recent calls list
    lines.push(Line::from(Span::styled(
        "⚡ Nedávná volání nástrojů:",
        Style::default().fg(Color::Cyan).bold(),
    )));
    if mcp.recent_calls.is_empty() {
        lines.push(Line::from(Span::styled(
            "   (žádná zaznamenaná volání)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for call in mcp.recent_calls.iter().rev().take(8) {
            let status = if call.is_error {
                Span::styled("✗ ", Style::default().fg(Color::Red).bold())
            } else {
                Span::styled("✓ ", Style::default().fg(Color::Green))
            };

            let badge_color = match call.badge.as_str() {
                "KB" => Color::Blue,
                "LSP" => Color::Magenta,
                "OR" => Color::Yellow,
                _ => Color::Cyan,
            };

            let mut row = vec![
                Span::raw("   "),
                status,
                Span::styled(
                    format!("[{}] ", call.badge),
                    Style::default().fg(badge_color).bold(),
                ),
                Span::styled(&call.tool, Style::default().fg(Color::White).bold()),
            ];

            if call.payload_tokens > 0 {
                row.push(Span::styled(
                    format!(" ({})", fmt_tokens(call.payload_tokens)),
                    Style::default().fg(Color::Cyan),
                ));
            }

            if !call.summary.is_empty() {
                row.push(Span::styled(
                    format!(" — {}", call.summary),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            lines.push(Line::from(row));
        }
    }

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta))
        .title(" MCP Servery & Protokol ");

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .scroll((state.scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}
