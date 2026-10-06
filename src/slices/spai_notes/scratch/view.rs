//! Scratchpad rendering (plan §1, §6): full-area editor, virtualised line
//! window, saved-line visuals (`✓ → project SPAI-014`, dim + italic).
use super::footer::render_footer;
use super::line_model::LineOrigin;
use super::super::state::SpaiNotesState;
use crate::shared::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph, Wrap},
    Frame,
};

pub fn render_scratch(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    theme::paint_backdrop(frame, area);
    let scratch = &state.scratch;

    let title = format!(
        " 📝 Scratchpad — {}{} ",
        scratch.scope.label(),
        if scratch.dirty { " ●" } else { "" },
    );
    // Yellow frame: the writing colour, matching the `. ` mark highlighting.
    let frame_color = Color::Rgb(255, 215, 0);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    let block = Block::bordered()
        .title(Span::styled(title, Style::default().fg(frame_color).bold()))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(frame_color));
    let inner = block.inner(rows[0]);
    frame.render_widget(block, rows[0]);

    render_lines(frame, inner, state);
    render_footer(frame, rows[1], state);

    // Dedup popup anchored under the cursor line (above the footer).
    super::dedup_popup::render_dedup_popup(frame, inner, state);
    // `@` project autocomplete while a mention token is open.
    super::mention_popup::render_mention_popup(frame, inner, state);

    // Fullscreen `?` help overlay sits above everything else.
    super::help::render_help_overlay(frame, area, state);
}

/// Visible window of lines that keeps the cursor on screen.
pub fn visible_window(total: usize, cursor: usize, height: usize) -> (usize, usize) {
    let height = height.max(1);
    if total <= height {
        return (0, total);
    }
    let scroll = cursor.saturating_sub(height.saturating_sub(1));
    (scroll.min(total.saturating_sub(height)), height)
}

fn render_lines(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    let scratch = &state.scratch;
    let inner_h = area.height as usize;
    let (scroll, _) = visible_window(scratch.lines.len(), scratch.cursor_line, inner_h);

    let mut lines: Vec<Line> = Vec::new();

    for (i, line) in scratch
        .lines
        .iter()
        .enumerate()
        .skip(scroll)
        .take(inner_h)
    {
        let is_cursor = i == scratch.cursor_line;
        lines.push(render_line(line, is_cursor, scratch.cursor_char));
    }

    if scratch.lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Pište — řádek se SPAI značkou (. / x ? - # * % …) = záznam; @projekt určí cíl.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  Ctrl+S uloží soubory · Ctrl+D duplicity · F1 nápověda",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let para = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(para, area);

    // Full-row highlight of the cursor line: painted directly onto the
    // buffer over the whole row width (Paragraph only styles its text
    // cells, a plain style set here covers the rest of the row too).
    if let Some(row_y) = cursor_row_y(area, state, scroll) {
        let row = Rect::new(area.x, row_y, area.width, 1);
        frame.buffer_mut().set_style(row, Style::default().bg(HIGHLIGHT_BG));
    }
}

/// The cursor-line highlight background (dark amber — readable with every
/// fg colour the highlighter uses, distinct from the yellow frame).
pub const HIGHLIGHT_BG: Color = Color::Rgb(58, 48, 8);

/// Buffer y of the cursor line inside this area, or None when off-screen.
fn cursor_row_y(area: Rect, state: &SpaiNotesState, scroll: usize) -> Option<u16> {
    let rel = state.scratch.cursor_line.checked_sub(scroll)?;
    if rel < area.height as usize {
        Some(area.y + rel as u16)
    } else {
        None
    }
}

/// One buffer line: every line renders in normal highlighting (records stay
/// fully workable after saving/loading); a record with a file gets a dim
/// `✓ → project id` label on its head only. Done (`x`) records keep their
/// struck-through styling via the highlighter's mark handling.
fn render_line(
    line: &super::line_model::ScratchLine,
    is_cursor: bool,
    cursor_char: usize,
) -> Line<'static> {
    let mut spans: Vec<Span> = Vec::new();

    // Every line is editable (bidirectional), so the cursor marker always
    // shows on the cursor line.
    let text = if is_cursor {
        insert_marker(&line.text, cursor_char)
    } else {
        line.text.clone()
    };

    spans.push(Span::styled("  ", Style::default()));
    spans.extend(highlight_spai_input_spans(&text));

    // The ✓ label belongs to the record head (the marked line); continuation
    // lines of the same file stay label-free.
    if let Some(origin) = line.origin.file_origin() {
        if super::line_model::is_marked(&line.text) {
            let label = format!(
                "  ✓ → {} {}",
                origin.project().unwrap_or(""),
                origin.record_id().unwrap_or("")
            );
            spans.push(Span::styled(
                label,
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    Line::from(spans)
}

fn insert_marker(text: &str, char_idx: usize) -> String {
    let at = text
        .char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    format!("{}█{}", &text[..at], &text[at..])
}

fn highlight_spai_input_spans(text: &str) -> Vec<Span<'static>> {
    super::super::input_highlighter::highlight_spai_input_spans(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_keeps_the_cursor_visible() {
        // Short buffer: everything visible.
        assert_eq!(visible_window(5, 0, 10), (0, 5));
        assert_eq!(visible_window(5, 4, 10), (0, 5));

        // Long buffer: the window follows the cursor.
        assert_eq!(visible_window(100, 0, 10), (0, 10));
        assert_eq!(visible_window(100, 50, 10), (41, 10));
        assert_eq!(visible_window(100, 99, 10), (90, 10));
    }
}
#[cfg(test)]
mod previews {
    use super::*;
    use crate::slices::spai_notes::discovery::SpaiProjectSummary;
    use crate::slices::spai_notes::state::SpaiNotesState;
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::PathBuf;

    fn draw(state: &SpaiNotesState, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).expect("terminal");
        t.draw(|f| render_scratch(f, f.area(), state)).expect("draw");
        let area = t.backend().buffer().area;
        let mut out = String::new();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                out.push_str(t.backend().buffer()[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn state_with_projects() -> SpaiNotesState {
        let mut state = SpaiNotesState::new(None);
        state.projects = vec![SpaiProjectSummary::new(
            "herdr".to_string(),
            PathBuf::from("D:/tmp/herdr"),
            PathBuf::from("D:/tmp/herdr/docs/spai"),
        )];
        state.current_project_path = Some(PathBuf::from("D:/tmp/herdr"));
        state
    }

    /// `cargo test scratch_preview_empty_edit -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn scratch_preview_empty_edit() {
        let mut state = state_with_projects();
        state.open_scratch(None);
        println!("{}", draw(&state, 90, 24));
    }

    /// `cargo test scratch_preview_typed_line -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn scratch_preview_typed_line() {
        let mut state = state_with_projects();
        state.open_scratch(None);
        for c in ". Opravit build @herdr".chars() {
            state.scratch.insert_char(c);
            state.scratch.cursor_char = state.scratch.current_line_len();
        }
        println!("{}", draw(&state, 90, 24));
    }

    /// `cargo test scratch_preview_after_save -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn scratch_preview_after_save() {
        let mut state = state_with_projects();
        state.open_scratch(None);
        // Simulate a saved line + one unsaved.
        state.scratch.lines = vec![
            crate::slices::spai_notes::scratch::line_model::ScratchLine {
                text: ". Fix build @herdr".to_string(),
                origin: crate::slices::spai_notes::scratch::line_model::LineOrigin::Saved {
                    path: PathBuf::from("D:/tmp/herdr/docs/spai/2026-10-05-SPAI-014-fix.md"),
                    id: "SPAI-014".to_string(),
                    project: "herdr".to_string(),
                    saved_text: ". Fix build @herdr".to_string(),
                },
            },
            crate::slices::spai_notes::scratch::line_model::ScratchLine {
                text: "? Nový nápad".to_string(),
                origin: crate::slices::spai_notes::scratch::line_model::LineOrigin::New,
            },
        ];
        state.scratch.cursor_line = 1;
        state.scratch.cursor_char = state.scratch.current_line_len();
        println!("{}", draw(&state, 90, 24));
    }

    /// `cargo test scratch_preview_dedup_popup -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn scratch_preview_dedup_popup() {
        use crate::slices::spai_notes::similarity::SimilarNoteMatch;
        let mut state = state_with_projects();
        state.open_scratch(None);
        for c in ". Opravit build @herdr".chars() {
            state.scratch.insert_char(c);
            state.scratch.cursor_char = state.scratch.current_line_len();
        }
        state.scratch.dedup = crate::slices::spai_notes::scratch::state::ScratchDedup {
            visible: true,
            anchor_line: 0,
            is_evaluating: false,
            matches: vec![SimilarNoteMatch {
                id: "SPAI-009".to_string(),
                title: "Opravit build pipeline".to_string(),
                symbol: ".".to_string(),
                similarity: 0.82,
                is_vector_match: true,
            }],
            selected: 0,
            receiver: None,
        };
        println!("{}", draw(&state, 90, 24));
    }

    /// `cargo test scratch_preview_mention_popup -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn scratch_preview_mention_popup() {
        let mut state = state_with_projects();
        state.projects.push(SpaiProjectSummary::new(
            "pi-spai".to_string(),
            PathBuf::from("D:/work/pi-spai"),
            PathBuf::from("D:/work/pi-spai/docs/spai"),
        ));
        state.open_scratch(None);
        for c in ". Opravit @her".chars() {
            state.scratch.insert_char(c);
            state.scratch.cursor_char = state.scratch.current_line_len();
        }
        state.scratch.update_mention_popup();
        println!("{}", draw(&state, 90, 24));
    }

    /// `cargo test scratch_preview_read_filtered -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn scratch_preview_read_filtered() {
        let mut state = state_with_projects();
        state.open_scratch(None);
        state.scratch.lines = vec![
            crate::slices::spai_notes::scratch::line_model::ScratchLine {
                text: ". Fix build @herdr".to_string(),
                origin: crate::slices::spai_notes::scratch::line_model::LineOrigin::FromFile {
                    path: PathBuf::from("D:/tmp/herdr/docs/spai/x.md"),
                    id: "SPAI-014".to_string(),
                    project: "herdr".to_string(),
                    loaded_text: ". Fix build @herdr".to_string(),
                },
            },
            crate::slices::spai_notes::scratch::line_model::ScratchLine {
                text: "? Jiný nápad".to_string(),
                origin: crate::slices::spai_notes::scratch::line_model::LineOrigin::FromFile {
                    path: PathBuf::from("D:/tmp/herdr/docs/spai/y.md"),
                    id: "SPAI-015".to_string(),
                    project: "herdr".to_string(),
                    loaded_text: "? Jiný nápad".to_string(),
                },
            },
            crate::slices::spai_notes::scratch::line_model::ScratchLine::empty(),
        ];
        state.scratch.filter =
            Some(crate::slices::spai_notes::scratch::filter::FilterQuery::parse("build"));
        println!("{}", draw(&state, 90, 24));
    }
}

#[cfg(test)]
mod highlight_tests {
    use super::*;
    use crate::slices::spai_notes::scratch::line_model::ScratchLine;
    use crate::slices::spai_notes::state::SpaiNotesState;
    use ratatui::{backend::TestBackend, Terminal};

    fn draw<'a>(state: &'a SpaiNotesState, w: u16, h: u16) -> ratatui::buffer::Buffer {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| render_scratch(f, f.area(), state)).unwrap();
        t.backend().buffer().clone()
    }

    #[test]
    fn cursor_line_background_covers_the_full_row() {
        let mut state = SpaiNotesState::new(None);
        state.open_scratch(None);
        state.scratch.lines = vec![
            ScratchLine { text: ". first".into(), origin: LineOrigin::New },
            ScratchLine { text: "? second record".into(), origin: LineOrigin::New },
        ];
        state.scratch.cursor_line = 1;
        state.scratch.cursor_char = 5;

        let backend = draw(&state, 60, 10);
        // Row of line 1 (buffer row 1): every cell carries the highlight bg,
        // including the space beyond the text (full-width highlight).
        for x in 1..59 {
            let cell = &backend[(x, 2)];
            assert_eq!(cell.bg, HIGHLIGHT_BG, "cell {x} bg: {:?}", cell.bg);
        }
        // The line above is NOT highlighted.
        assert_ne!(backend[(5, 1)].bg, HIGHLIGHT_BG);
    }

    #[test]
    fn highlight_bg_is_readable_with_highlighter_colors() {
        // Dark amber: every fg the highlighter uses stays clearly visible.
        let fg_colors = [
            Color::Rgb(241, 252, 121), // Úkol badge
            Color::Rgb(255, 215, 0),   // task yellow
            Color::Rgb(186, 85, 211),  // idea purple
            Color::Rgb(127, 179, 255), // note blue
            Color::Rgb(144, 238, 144), // done green
            Color::White,
            Color::DarkGray,
        ];
        for fg in fg_colors {
            let contrast = |a: [u8; 3], b: [u8; 3]| -> f64 {
                let lum = |c: [u8; 3]| -> f64 {
                    0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64
                };
                (lum(a) - lum(b)).abs()
            };
            let bg = [58, 48, 8];
            let fgc = match fg {
                Color::Rgb(r, g, b) => [r, g, b],
                Color::White => [255, 255, 255],
                Color::DarkGray => [128, 128, 128],
                _ => continue,
            };
            assert!(contrast(fgc, bg) > 60.0, "fg {fg:?} too close to bg");
        }
    }
}
