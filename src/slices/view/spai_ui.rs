//! SPAI ledger face lines.
use crate::slices::view::state::SidebarState;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};

/// SPAI ledger lines (Linkarzu TrueColor ribbon + counts + open tasks).
pub fn render_spai_lines(state: &SidebarState) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::raw(""));
    let spai_title_spans = vec![Span::styled(
        "SPAI:    ",
        Style::default().fg(Color::DarkGray),
    )];

    if let Some(spai) = &state.spai {
        let counts = &spai.counts;
        let mut ribbon_spans = spai_title_spans;

        // Render multi-segment ribbon: done (mint) | working (yellow) | waiting (violet) | todo (pink) | cancelled (slate)
        let ribbon_len = 16usize;
        let total = counts.total_tasks;

        if total > 0 {
            let seg = |count: usize| -> usize {
                ((count as f64 / total as f64) * ribbon_len as f64).round() as usize
            };

            let s_done = seg(counts.done);
            let s_work = seg(counts.working);
            let s_wait = seg(counts.waiting);
            let s_cancel = seg(counts.cancelled);
            let s_todo = ribbon_len.saturating_sub(s_done + s_work + s_wait + s_cancel);

            // Truecolors from SPAI Linkarzu Palette
            let col_done = Color::Rgb(55, 244, 153); // #37f499 neon mint
            let col_work = Color::Rgb(241, 252, 121); // #f1fc79 electric yellow
            let col_wait = Color::Rgb(152, 122, 251); // #987afb neon violet
            let col_todo = Color::Rgb(249, 77, 255); // #f94dff vivid pink
            let col_cancel = Color::Rgb(135, 145, 170); // #8791aa slate

            if s_done > 0 {
                ribbon_spans.push(Span::styled(
                    "█".repeat(s_done),
                    Style::default().fg(col_done),
                ));
            }
            if s_work > 0 {
                ribbon_spans.push(Span::styled(
                    "█".repeat(s_work),
                    Style::default().fg(col_work),
                ));
            }
            if s_wait > 0 {
                ribbon_spans.push(Span::styled(
                    "█".repeat(s_wait),
                    Style::default().fg(col_wait),
                ));
            }
            if s_todo > 0 {
                ribbon_spans.push(Span::styled(
                    "█".repeat(s_todo),
                    Style::default().fg(col_todo),
                ));
            }
            if s_cancel > 0 {
                ribbon_spans.push(Span::styled(
                    "░".repeat(s_cancel),
                    Style::default().fg(col_cancel),
                ));
            }

            let pct = (counts.done as f64 / total as f64 * 100.0).round() as usize;
            ribbon_spans.push(Span::styled(
                format!(" [{}/{}] {}%", counts.done, total, pct),
                Style::default().fg(col_done).bold(),
            ));
            lines.push(Line::from(ribbon_spans));

            // Breakdown counts
            let mut stat_spans = vec![Span::raw("         ")];
            if counts.working > 0 {
                stat_spans.push(Span::styled(
                    format!("◐ {} běží  ", counts.working),
                    Style::default().fg(col_work),
                ));
            }
            if counts.waiting > 0 {
                stat_spans.push(Span::styled(
                    format!("⏳ {} čeká  ", counts.waiting),
                    Style::default().fg(col_wait),
                ));
            }
            if counts.todo > 0 {
                stat_spans.push(Span::styled(
                    format!("○ {} úkolů  ", counts.todo),
                    Style::default().fg(col_todo),
                ));
            }
            if counts.ideas > 0 {
                stat_spans.push(Span::styled(
                    format!("💡 {} nápadů", counts.ideas),
                    Style::default().fg(Color::Rgb(4, 209, 249)), // Cyan
                ));
            }
            lines.push(Line::from(stat_spans));

            // List 2-3 most recent or working tasks
            let mut shown = 0;
            // Prioritize working, waiting, and open todo
            for r in spai.index.records.iter().rev() {
                let st = r.status.to_lowercase();
                if st == "working" || st == "waiting" || st == "todo" {
                    let (icon, color) = match st.as_str() {
                        "working" => ("◐", col_work),
                        "waiting" => ("⏳", col_wait),
                        _ => ("○", col_todo),
                    };

                    let title = if r.title.len() > 36 {
                        format!("{}…", &r.title[..36])
                    } else {
                        r.title.clone()
                    };

                    lines.push(Line::from(vec![
                        Span::raw("         "),
                        Span::styled(format!("{} ", icon), Style::default().fg(color).bold()),
                        Span::styled(format!("{}: ", r.id), Style::default().fg(Color::DarkGray)),
                        Span::styled(title, Style::default().fg(Color::Gray)),
                    ]));
                    shown += 1;
                    if shown >= 3 {
                        break;
                    }
                }
            }
        } else {
            ribbon_spans.push(Span::styled(
                "žádné úkoly v docs/spai",
                Style::default().fg(Color::DarkGray),
            ));
            lines.push(Line::from(ribbon_spans));
        }
    } else {
        let mut no_spai_spans = spai_title_spans;
        no_spai_spans.push(Span::styled(
            "bez docs/spai ledgeru",
            Style::default().fg(Color::DarkGray),
        ));
        lines.push(Line::from(no_spai_spans));
    }

    lines
}
