//! Bottom shared banner shown above the shortcuts footer on every face:
//! model/thinking, context gauge, token economics, quota and credits.
//!
//! Split into one module per line group so each file stays reviewable; the
//! public surface (`shared_banner_height`, `render_shared_model_banner`) is
//! unchanged for `view::ui`.

use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

use crate::slices::telemetry::fmt_tokens;
use crate::slices::view::state::SidebarState;

mod context_lines;
mod credits_lines;
mod model_lines;

use context_lines::build_context_and_cost_lines;
use credits_lines::build_quota_and_credits_lines;
use model_lines::build_model_and_turns_lines;

pub fn shared_banner_height(state: &SidebarState) -> u16 {
    if state.active_tab == crate::slices::view::state::Tab::Notes {
        return 1;
    }

    let mut base_lines = 4;
    if let Some(l) = &state.live {
        let prompt = l.input_tokens + l.cache_read + l.cache_write;
        if prompt > 0 && l.cache_read > 0 {
            base_lines += 1;
        }
    }
    if let Some(q) = &state.quota {
        if q.antigravity.is_some() {
            base_lines += 1;
        }
    }
    let or_lines = if let Some(or) = &state.openrouter_credits {
        if or.accounts.is_empty() {
            0
        } else if or.accounts.len() == 1 {
            1
        } else {
            1 + or.accounts.len()
        }
    } else {
        0
    };
    (base_lines + or_lines + 2) as u16
}

pub fn render_shared_model_banner(frame: &mut Frame, area: Rect, state: &SidebarState) {
    if state.active_tab == crate::slices::view::state::Tab::Notes {
        render_minimalist_notes_banner(frame, area, state);
        return;
    }

    let mut lines = Vec::new();
    lines.extend(build_model_and_turns_lines(state));
    lines.extend(build_context_and_cost_lines(state));
    lines.extend(build_quota_and_credits_lines(state));

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Model & Kredity ");

    let paragraph = Paragraph::new(Text::from(lines)).block(block);
    frame.render_widget(paragraph, area);
}

fn render_minimalist_notes_banner(frame: &mut Frame, area: Rect, state: &SidebarState) {
    let live = state.live.as_ref();
    let model = live
        .map(|l| {
            if l.model_id.is_empty() {
                "neznámý model".to_string()
            } else {
                l.model_id.clone()
            }
        })
        .unwrap_or_else(|| "offline".to_string());

    let provider = live
        .map(|l| l.provider.as_str())
        .filter(|p| !p.is_empty())
        .unwrap_or("pi");

    let ctx_pct = live.and_then(|l| l.context_percent).unwrap_or(0.0);
    let ctx_tokens = live.map(|l| l.context_tokens).unwrap_or(0);
    let ctx_window = live.map(|l| l.context_window).unwrap_or(0);
    let ctx_known = live.is_some_and(|l| !l.context_unknown) && ctx_window > 0;

    let pct_color = if !ctx_known {
        Color::DarkGray
    } else if ctx_pct >= 90.0 {
        Color::Red
    } else if ctx_pct >= 60.0 {
        Color::Yellow
    } else {
        Color::Green
    };

    let line = Line::from(vec![
        Span::styled(" 🤖 ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}/", provider),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(model, Style::default().fg(Color::Cyan).bold()),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled("📊 ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if ctx_known {
                format!("{:.1}%", ctx_pct)
            } else {
                "?".to_string()
            },
            Style::default().fg(pct_color).bold(),
        ),
        Span::styled(
            if ctx_known {
                format!(" ({}/{})", fmt_tokens(ctx_tokens), fmt_tokens(ctx_window))
            } else if ctx_window > 0 {
                format!(" (?/{})", fmt_tokens(ctx_window))
            } else {
                String::new()
            },
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}
