//! Fullscreen `?` help overlay for the Scratchpad (both modes).
//! Render-only; interaction comes from the key layer (any key closes it).
use super::super::state::SpaiNotesState;
use crate::shared::theme;
use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Wrap},
    Frame,
};

/// Two spans per row (key + desc) so rows stay consistent.
fn row(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" [{key}] "),
            Style::default().fg(Color::Yellow).bold(),
        ),
        Span::styled(desc.to_string(), Style::default().fg(Color::DarkGray)),
    ])
}

/// A dim section header.
fn head(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(Color::Cyan).bold(),
    ))
}

fn blank() -> Line<'static> {
    Line::from("")
}

fn help_lines() -> Vec<Line<'static>> {
    vec![
        head("ZÁKLAD"),
        row("Ctrl+N", "otevřít zápisník (z kteréhokoli tabu)"),
        row("Esc", "zavřít (2× při neuložených — draft zůstává)"),
        row("F1", "tahle nápověda"),
        blank(),
        head("PSANÍ (vždy — jeden mód)"),
        row(". x ? - … + mezera", "rozpozná typ → vloží [10.2.2026 14:22:37]"),
        row("@", "popup projektů — ↑/↓ vybrat, Enter/Tab doplnit, Esc skrýt"),
        row("Enter", "nový řádek (řádky bez značky = pokračování záznamu)"),
        row("Ctrl+S", "uložit/aktualizovat: řádky ↔ soubory, ✓ → projekt SPAI-xxx"),
        blank(),
        head("ZÁZNAMY"),
        row("Ctrl+O", "otevřít záznam pod kurzorem v editoru Notes"),
        row("Ctrl+X", "cyklovat stav záznamu (rovnou na disku)"),
        row("Ctrl+Z", "vrátit poslední Ctrl+S dávku (smaže soubory)"),
        row("Ctrl+Del", "smazat záznam (soubor + řádky; 2× potvrzení)"),
        row("Ctrl+D", "duplicity pod aktuálním řádkem (jen na vyžádání)"),
        blank(),
        head("SCOPE A FILTRY"),
        row("Ctrl+T", "cyklovat scope: Nové → Projekt → Vše"),
        row("Ctrl+F", "fuzzy filtr: text @projekt :tag: !priorita"),
        row("Ctrl+R", "sémantický filtr (embedding + kosiny, po Enter)"),
        row("Ctrl+Y", "cyklovat stavový filtr (vše → otevřené → hotové)"),
        row("Ctrl+L", "zrušit všechny filtry"),
        blank(),
        head("POPUPOVERY"),
        row("↑/↓", "vybrat položku (duplicity i projekty)"),
        row("Ctrl+O", "otevřít vybranou duplicitu"),
        row("Ctrl+A", "připojit text k duplicitnímu záznamu"),
        row("Ctrl+U", "cyklovat stav duplicity"),
        row("Esc", "zavřít popup"),
    ]
}

pub fn render_help_overlay(frame: &mut Frame, area: Rect, state: &SpaiNotesState) {
    if !state.scratch.help_visible {
        return;
    }
    theme::paint_backdrop(frame, area);

    let lines = help_lines();
    let width = area.width.saturating_sub(4).min(82);
    let height = lines.len() as u16 + 2; // + borders
    let height = height.min(area.height.saturating_sub(2));
    if width == 0 || height == 0 {
        return;
    }
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);

    let block = Block::bordered()
        .title(Span::styled(
            " ❓ Scratchpad — nápověda (libovolná klávesa zavře) ",
            Style::default().fg(Color::Cyan).bold(),
        ))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(Color::DarkGray))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slices::spai_notes::scratch::line_model::ScratchLine;
    use crate::slices::spai_notes::state::SpaiNotesState;
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::PathBuf;

    /// `cargo test help_preview -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn help_preview() {
        let mut state = SpaiNotesState::new(None);
        state.projects = vec![crate::slices::spai_notes::discovery::SpaiProjectSummary::new(
            "herdr".to_string(),
            PathBuf::from("D:/tmp/herdr"),
            PathBuf::from("D:/tmp/herdr/docs/spai"),
        )];
        state.open_scratch(None);
        state.scratch.lines = vec![ScratchLine::empty()];
        state.scratch.help_visible = true;
        let mut t = Terminal::new(TestBackend::new(90, 30)).unwrap();
        t.draw(|f| super::render_help_overlay(f, f.area(), &state)).unwrap();
        let area = t.backend().buffer().area;
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                print!("{}", t.backend().buffer()[(x, y)].symbol());
            }
            println!();
        }
    }

    #[test]
    fn help_rows_are_complete() {
        let lines = help_lines();
        let text: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.clone()).collect())
            .collect();
        let joined = text.join("\n");
        for must in [
            "Ctrl+T",
            "Ctrl+N",
            "Ctrl+S",
            "Ctrl+D",
            "Ctrl+O",
            "sémantický filtr",
            "vrátit poslední Ctrl+S dávku",
        ] {
            assert!(joined.contains(must), "missing: {must}");
        }
    }
}
