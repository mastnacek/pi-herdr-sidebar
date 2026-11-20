//! Czech title of a record (`title_cs`, written by `spai`) and its upkeep.
//! The parser keeps `title_cs`, `lang` and `pending` verbatim in
//! `extra_frontmatter`; this module reads and adjusts them.
use super::note::SpaiNoteItem;
use super::storage_format::parser::unescape_yaml_scalar;

const EXTRA_TITLE_CS: &str = "title_cs";
const EXTRA_PENDING: &str = "pending";
const TRANSLATE_CS: &str = "translate_cs";

impl SpaiNoteItem {
    /// The Czech title, when the record has one.
    pub fn title_cs(&self) -> Option<String> {
        let (_, raw) = self.extra_frontmatter.iter().find(|(k, _)| k == EXTRA_TITLE_CS)?;
        let raw = raw.trim();
        let raw = raw.strip_prefix('"').unwrap_or(raw);
        let t = unescape_yaml_scalar(raw.strip_suffix('"').unwrap_or(raw));
        (!t.trim().is_empty()).then_some(t)
    }

    /// What the UI shows: the Czech title, else the title.
    pub fn display_title(&self) -> String {
        self.title_cs().unwrap_or_else(|| self.title.clone())
    }

    /// After the (English) title was edited: a changed title makes the Czech
    /// one stale, so it is dropped and `translate_cs` is added to `pending`
    /// (`spai reindex` fills it in). An unchanged title keeps everything.
    pub fn on_title_changed(&mut self, old_title: &str) {
        if self.title == old_title || self.title_cs().is_none() {
            return;
        }
        self.extra_frontmatter.retain(|(k, _)| k != EXTRA_TITLE_CS);
        match self.extra_frontmatter.iter_mut().find(|(k, _)| k == EXTRA_PENDING) {
            Some((_, v)) => {
                let mut items: Vec<String> = v
                    .trim()
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !items.iter().any(|s| s == TRANSLATE_CS) {
                    items.push(TRANSLATE_CS.to_string());
                }
                *v = format!("[{}]", items.join(", "));
            }
            None => self
                .extra_frontmatter
                .push((EXTRA_PENDING.to_string(), format!("[{TRANSLATE_CS}]"))),
        }
    }
}

#[cfg(test)]
mod tests;
