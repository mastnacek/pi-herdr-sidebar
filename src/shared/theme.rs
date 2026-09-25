//! Shared modal styling.
//!
//! Every modal in this plugin (SPAI note editor, plugin-usage stats, ...) draws
//! the same three-level hierarchy so they read consistently as a floating window
//! instead of dissolving into the panels underneath:
//!
//! 1. [`MODAL_BG`] — raised backdrop painted over the whole modal rect.
//! 2. [`FIELD_BG_ACTIVE`] — near-black inset for the sub-area that owns focus.
//! 3. default background — everything that is neither.
//!
//! This lives in the kernel because the styling is a cross-cutting concern:
//! slices must never import each other's presentation helpers.
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Clear},
    Frame,
};

/// Raised backdrop painted behind a modal so it reads as its own window.
pub const MODAL_BG: Color = Color::Rgb(30, 30, 40);

/// Inset fill for the focused field — deliberately darker than [`MODAL_BG`] so
/// the active area is obvious at a glance.
pub const FIELD_BG_ACTIVE: Color = Color::Rgb(10, 10, 14);

/// Accent used for modal key hints and section headers.
pub const MODAL_ACCENT: Color = Color::Rgb(139, 233, 253);

/// Centres a rect covering `percent_x`% × `percent_y`% of `area`.
pub fn centered_percent(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

/// Clears `area` and fills it with [`MODAL_BG`], making the modal prominent
/// against whatever the terminal shows behind it.
pub fn paint_backdrop(frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    frame.render_widget(Block::default().style(Style::default().bg(MODAL_BG)), area);
}

/// Background for a focusable sub-area: inset when focused, backdrop otherwise.
pub fn field_bg(focused: bool) -> Color {
    if focused {
        FIELD_BG_ACTIVE
    } else {
        MODAL_BG
    }
}

/// Block title with a leading focus marker. The marker is a non-colour cue, so
/// the focused field stays identifiable without relying on colour alone.
pub fn field_title(label: &str, focused: bool) -> String {
    if focused {
        format!(" ▶ {label} ")
    } else {
        format!("   {label} ")
    }
}

/// Renders a section header line (`▌ Title`) used inside modals.
pub fn section_header(title: &str) -> ratatui::text::Line<'static> {
    use ratatui::{
        style::Stylize,
        text::{Line, Span},
    };
    Line::from(vec![
        Span::styled("▌ ", Style::default().fg(MODAL_ACCENT)),
        Span::styled(title.to_string(), Style::default().fg(Color::White).bold()),
    ])
}

/// A proportional bar (`████░░░░`) sized to `value`/`max` for `width` cells.
pub fn usage_bar(value: u64, max: u64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let ratio = if max == 0 {
        0.0
    } else {
        value as f64 / max as f64
    };
    let filled = ((ratio * width as f64).round() as usize).clamp(0, width);
    let mut bar = "█".repeat(filled);
    bar.push_str(&"░".repeat(width - filled));
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_field_is_the_dark_one() {
        assert_eq!(field_bg(true), FIELD_BG_ACTIVE);
        assert_eq!(field_bg(false), MODAL_BG);
        assert_ne!(FIELD_BG_ACTIVE, MODAL_BG);
    }

    #[test]
    fn focus_marker_tracks_focus() {
        assert!(field_title("Název", true).starts_with(" ▶ "));
        assert!(field_title("Název", false).starts_with("   "));
    }

    #[test]
    fn bars_are_exact_width_and_clamped() {
        assert_eq!(usage_bar(0, 10, 4), "░░░░");
        assert_eq!(usage_bar(10, 10, 4), "████");
        assert_eq!(usage_bar(5, 10, 4), "██░░");
        assert_eq!(usage_bar(99, 10, 4), "████", "over-max is clamped");
        assert_eq!(usage_bar(1, 0, 4), "░░░░", "no division by zero");
        assert_eq!(usage_bar(1, 1, 0), "");
    }

    #[test]
    fn centered_rect_stays_inside_area() {
        let area = Rect::new(0, 0, 100, 40);
        let r = centered_percent(area, 90, 80);
        assert!(r.x >= area.x && r.y >= area.y);
        assert!(r.right() <= area.right() && r.bottom() <= area.bottom());
        assert!(r.width > 0 && r.height > 0);
    }
}
