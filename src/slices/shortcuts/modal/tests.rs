//! Render tests for the standalone usage overview, using ratatui's `TestBackend`
//! so layout, content and the modal backdrop are asserted on real cells.
use super::*;
use crate::slices::shortcuts::usage::model::{Counted, PluginUsage};
use crate::slices::shortcuts::usage::state::UsageOverview;
use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

fn sample_stats() -> UsageStats {
    UsageStats {
        files: 430,
        messages: 72_111,
        tool_calls: 33_882,
        builtin_calls: 29_931,
        plugins: vec![
            PluginUsage {
                plugin: "builtin".into(),
                calls: 29_931,
                tools: vec![("bash".into(), 15_043)],
            },
            PluginUsage {
                plugin: "context-mode".into(),
                calls: 1_389,
                tools: vec![("ctx_execute".into(), 933)],
            },
            PluginUsage {
                plugin: "pi-lens".into(),
                calls: 562,
                tools: vec![("lens_diagnostics".into(), 365)],
            },
        ],
        tools: vec![
            Counted {
                name: "bash".into(),
                count: 15_043,
            },
            Counted {
                name: "ctx_execute".into(),
                count: 933,
            },
        ],
        skills: vec![Counted {
            name: "pi-plugin-dev".into(),
            count: 95,
        }],
        commands: vec![Counted {
            name: "/goal".into(),
            count: 12,
        }],
        elapsed_ms: 342,
        newest_session: None,
    }
}

fn state_with_stats() -> UsageOverview {
    let mut state = UsageOverview::new();
    state.stats = Some(sample_stats());
    state
}

/// Renders the panel exactly as the overview window does: full area, no chrome.
fn render_panel(state: &UsageOverview, width: u16, height: u16) -> Buffer {
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
fn panel_shows_every_section_and_fills_the_window_with_the_backdrop() {
    let buffer = render_panel(&state_with_stats(), 120, 40);
    let text = buffer_text(&buffer);

    assert!(text.contains("Využití pluginů a skillů"), "title missing");
    assert!(text.contains("Pluginy"), "plugin column missing");
    assert!(text.contains("Skilly"), "skill column missing");
    assert!(text.contains("Příkazy"), "command column missing");
    assert!(text.contains("Nejčastější nástroje"), "tool row missing");
    assert!(text.contains("context-mode"), "plugin rows missing");
    assert!(text.contains("pi-plugin-dev"), "skill rows missing");
    assert!(
        text.contains("1\u{202f}389"),
        "thousands grouping missing: {text}"
    );

    // The whole window is the modal backdrop, not just an inner dialog rect.
    let total = 120 * 40;
    let backdrop = backdrop_cells(&buffer);
    assert!(
        backdrop > total / 2,
        "backdrop should cover most of the window: {backdrop}/{total}"
    );
}

#[test]
fn panel_is_standalone_and_shows_no_sidebar_chrome() {
    let text = buffer_text(&render_panel(&state_with_stats(), 120, 40));
    // Sidebar-only furniture must not appear in the overview window.
    assert!(!text.contains("Zkratky"), "no Shortcuts tab header");
    assert!(!text.contains("0: Zen"), "no tab bar");
    assert!(!text.contains("Model & Kredity"), "no shared banner");
}

#[test]
fn panel_shows_empty_hint_without_data() {
    let text = buffer_text(&render_panel(&UsageOverview::new(), 100, 30));
    assert!(text.contains("Zatím žádná data"), "empty hint missing");
    assert!(text.contains("Využití pluginů"), "title still shown");
}

#[test]
fn panel_survives_a_small_window() {
    let text = buffer_text(&render_panel(&state_with_stats(), 46, 12));
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
