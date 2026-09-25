//! Tests for the usage overview document and its scrolling, using ratatui's
//! `TestBackend` so layout, content and the modal backdrop are asserted on real
//! cells.
use super::*;
use crate::slices::shortcuts::usage::model::{Counted, PluginUsage};
use crate::slices::shortcuts::usage::UsageStats;
use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

/// Fixture with `n` items per section; names stay shorter than `NAME_W` so the
/// only ellipsis that could appear would be a bug.
fn stats_with(plugins: usize, skills: usize, commands: usize, tools: usize) -> UsageStats {
    UsageStats {
        files: 430,
        messages: 72_111,
        tool_calls: 33_882,
        builtin_calls: 29_931,
        plugins: (0..plugins)
            .map(|i| PluginUsage {
                plugin: format!("plugin-{i:02}"),
                calls: (plugins - i) as u64,
                tools: vec![(format!("tool-{i:02}"), 1)],
            })
            .collect(),
        tools: (0..tools)
            .map(|i| Counted {
                name: format!("tool-{i:02}"),
                count: (tools - i) as u64,
            })
            .collect(),
        skills: (0..skills)
            .map(|i| Counted {
                name: format!("skill-{i:02}"),
                count: (skills - i) as u64,
            })
            .collect(),
        commands: (0..commands)
            .map(|i| Counted {
                name: format!("cmd-{i:02}"),
                count: 1,
            })
            .collect(),
        elapsed_ms: 342,
        newest_session: None,
    }
}

fn state_with(stats: UsageStats) -> UsageOverview {
    let mut state = UsageOverview::new();
    state.stats = Some(stats);
    state
}

fn render_panel(state: &mut UsageOverview, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| render_usage_panel(frame, frame.area(), state))
        .expect("draw");
    terminal.backend().buffer().clone()
}

fn buffer_text(buffer: &Buffer) -> String {
    let area = buffer.area;
    let mut out = String::new();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn document_text(doc: &[Line]) -> String {
    doc.iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn backdrop_cells(buffer: &Buffer) -> usize {
    let area = buffer.area;
    let mut count = 0;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if buffer[(x, y)].bg == theme::MODAL_BG {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn document_contains_every_item_and_truncates_nothing() {
    let stats = stats_with(30, 30, 30, 40);
    let text = document_text(&document(&stats));

    for i in 0..30 {
        assert!(
            text.contains(&format!("plugin-{i:02}")),
            "missing plugin {i}"
        );
        assert!(text.contains(&format!("skill-{i:02}")), "missing skill {i}");
        assert!(text.contains(&format!("cmd-{i:02}")), "missing command {i}");
    }
    for i in 0..40 {
        assert!(text.contains(&format!("tool-{i:02}")), "missing tool {i}");
    }

    // The old layout cut lists off with a "… +N dalších" row; that must be gone.
    assert!(!text.contains("dalších"), "no 'and N more' rows");
    assert!(!text.contains('…'), "nothing is elided in the document");
}

#[test]
fn document_has_a_header_per_section_with_counts() {
    let stats = stats_with(4, 3, 2, 5);
    let text = document_text(&document(&stats));
    assert!(text.contains("Pluginy (4)"));
    assert!(text.contains("Skilly (3)"));
    assert!(text.contains("Příkazy (2)"));
    assert!(text.contains("Nástroje (5)"));
}

#[test]
fn viewport_scrolls_to_reach_the_last_item() {
    let mut state = state_with(stats_with(40, 0, 0, 0));

    // First paint: the viewport shows the top of the document.
    let top = buffer_text(&render_panel(&mut state, 100, 20));
    assert!(top.contains("plugin-00"), "first item visible at the top");
    assert!(!top.contains("plugin-39"), "last item is below the fold");

    // End jumps to the bottom, where the remaining items are reachable.
    state.scroll_end();
    let bottom = buffer_text(&render_panel(&mut state, 100, 20));
    assert!(
        bottom.contains("plugin-39"),
        "last item reachable by scrolling"
    );
    assert!(bottom.contains("řádky"), "footer reports the position");
}

#[test]
fn paging_walks_the_whole_document() {
    let mut state = state_with(stats_with(60, 0, 0, 0));
    render_panel(&mut state, 100, 20);

    assert_eq!(state.scroll, 0);
    // A page is one screen minus the overlap row, where the screen height comes
    // from the layout (title 3 + footer 2 are subtracted from the window).
    let page = state.view_h - 1;
    state.page(true);
    assert_eq!(state.scroll, page);

    let mut seen = 0;
    while state.scroll < state.max_scroll() {
        state.page(true);
        seen += 1;
        assert!(seen < 200, "paging must terminate");
    }
    assert_eq!(state.scroll, state.max_scroll(), "reaches the very end");
}

#[test]
fn rendering_keeps_the_scroll_inside_the_document() {
    let mut state = state_with(stats_with(5, 0, 0, 0));
    state.scroll = 9_999;
    render_panel(&mut state, 100, 20);
    assert!(
        state.scroll <= state.max_scroll(),
        "a stale scroll offset is clamped on render"
    );
}

#[test]
fn panel_fills_the_window_with_the_backdrop() {
    let mut state = state_with(stats_with(6, 4, 3, 8));
    let buffer = render_panel(&mut state, 120, 40);
    let text = buffer_text(&buffer);

    assert!(text.contains("Využití pluginů a skillů"), "title missing");
    assert!(text.contains("plugin-00"), "plugin rows missing");
    assert!(text.contains("skill-00"), "skill rows missing");
    assert!(text.contains("cmd-00"), "command rows missing");
    assert!(text.contains("tool-00"), "tool rows missing");

    let total = 120 * 40;
    assert!(
        backdrop_cells(&buffer) > total / 2,
        "backdrop should cover most of the window"
    );
}

#[test]
fn scroll_position_stays_visible_on_a_narrow_pane() {
    // The totals string alone used to fill the footer and push the position
    // indicator off-screen; it now lives in its own right-aligned column.
    let mut state = state_with(stats_with(40, 10, 10, 20));
    let text = buffer_text(&render_panel(&mut state, 80, 20));
    assert!(
        text.contains("řádky"),
        "position indicator must survive a narrow footer:\n{text}"
    );
}

#[test]
fn panel_stays_standalone_without_sidebar_chrome() {
    let mut state = state_with(stats_with(3, 1, 1, 2));
    let text = buffer_text(&render_panel(&mut state, 120, 40));
    assert!(!text.contains("Zkratky"), "no Shortcuts tab header");
    assert!(!text.contains("0: Zen"), "no tab bar");
    assert!(!text.contains("Model & Kredity"), "no shared banner");
}

#[test]
fn panel_shows_empty_hint_without_data() {
    let mut state = UsageOverview::new();
    let text = buffer_text(&render_panel(&mut state, 100, 30));
    assert!(text.contains("Zatím žádná data"), "empty hint missing");
    assert!(text.contains("Využití pluginů"), "title still shown");
}

#[test]
fn panel_survives_a_small_window() {
    let mut state = state_with(stats_with(20, 20, 20, 20));
    let text = buffer_text(&render_panel(&mut state, 46, 12));
    assert!(!text.is_empty());
}

#[test]
fn groups_thousands() {
    assert_eq!(format_count(0), "0");
    assert_eq!(format_count(999), "999");
    assert_eq!(format_count(1_000), "1\u{202f}000");
    assert_eq!(format_count(70_627), "70\u{202f}627");
    assert_eq!(format_count(1_234_567), "1\u{202f}234\u{202f}567");
}
