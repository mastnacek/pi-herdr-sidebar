#[cfg(test)]
mod previews {
    use super::super::view::{render_scratch, visible_window};
    use crate::slices::spai_notes::scratch::line_model::{LineOrigin, ScratchLine};
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
    use ratatui::style::Color;
    use super::super::view::render_scratch;
    use crate::slices::spai_notes::scratch::line_render::HIGHLIGHT_BG;
    use crate::slices::spai_notes::scratch::line_model::{LineOrigin, ScratchLine};
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
