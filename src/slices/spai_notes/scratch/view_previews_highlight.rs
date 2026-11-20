#[cfg(test)]
mod highlight_tests {
    use super::super::view::render_scratch;
    use crate::slices::spai_notes::scratch::line_model::{LineOrigin, ScratchLine};
    use crate::slices::spai_notes::scratch::line_render::HIGHLIGHT_BG;
    use crate::slices::spai_notes::state::SpaiNotesState;
    use ratatui::style::Color;
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
            ScratchLine {
                text: ". first".into(),
                origin: LineOrigin::New,
            },
            ScratchLine {
                text: "? second record".into(),
                origin: LineOrigin::New,
            },
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
    fn long_lines_do_not_shift_the_highlight_or_cursor() {
        // A 200-char first line used to wrap and push every visual row down,
        // shifting the highlight band off the cursor. With wrapping disabled
        // the band must sit exactly on the cursor's visual row.
        let mut state = SpaiNotesState::new(None);
        state.open_scratch(None);
        state.scratch.lines = vec![
            ScratchLine {
                text: "x ".to_string() + "dlouha radka ".repeat(16).trim(),
                origin: LineOrigin::New,
            },
            ScratchLine {
                text: "? druhy".to_string(),
                origin: LineOrigin::New,
            },
        ];
        state.scratch.cursor_line = 1;
        state.scratch.cursor_char = 0;
        let buf = draw(&state, 60, 10);
        // Cursor row is buffer row 2 (inside the border) — highlighted.
        assert_eq!(buf[(5, 2)].bg, HIGHLIGHT_BG);
        // The inverted cursor cell sits in the FIRST character space (col 2):
        // reversed modifier, no shifted text.
        let cell = &buf[(3, 2)];
        assert!(cell.modifier.contains(ratatui::style::Modifier::REVERSED));
        // The line above is NOT highlighted.
        assert_ne!(buf[(5, 1)].bg, HIGHLIGHT_BG);
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

