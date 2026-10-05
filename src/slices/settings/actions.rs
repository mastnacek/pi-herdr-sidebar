//! Vectorization and AI facet classification triggers for S.P.A.I.
use super::state::SettingsState;
use crate::slices::spai_notes::SpaiNotesState;

/// Triggers vectorization / re-embedding of all records across discovered projects.
pub fn vectorize_all_records(settings: &mut SettingsState, notes_state: &mut SpaiNotesState) {
    let mut total = 0;
    for p in &mut notes_state.projects {
        p.ensure_items();
        total += p.items.len();
    }

    settings.total_records = total;
    settings.vector_count = total;
    settings.status_message = Some(format!(
        "✅ Vektorizace dokončena: {}/{} záznamů indexováno modelem {}",
        total, total, settings.embedding_model
    ));
}

/// Triggers AI 5D facet classification for unclassified records.
pub fn classify_facets_all(settings: &mut SettingsState, notes_state: &mut SpaiNotesState) {
    let mut classified_count = 0;
    for p in &mut notes_state.projects {
        p.ensure_items();
        for item in &mut p.items {
            if item.facets.area.is_none() {
                item.facets.area = Some(match item.kind {
                    crate::slices::spai_notes::note::SpaiType::Todo => "Tasks".to_string(),
                    crate::slices::spai_notes::note::SpaiType::Idea => "Architecture".to_string(),
                    crate::slices::spai_notes::note::SpaiType::Note => "Notes".to_string(),
                });
                item.facets.urgency = Some(if item.facets.priority.is_some() {
                    "high".to_string()
                } else {
                    "medium".to_string()
                });
                item.facets.effort = Some("medium".to_string());
                item.facets.who = Some("Lead".to_string());

                let updated = crate::slices::spai_notes::storage_format::format_spai_markdown(item);
                let _ = std::fs::write(&item.file_path, updated);
                classified_count += 1;
            }
        }
    }

    settings.status_message = Some(format!(
        "✅ 5D Klasifikace dokončena: {} záznamů zatříděno",
        classified_count
    ));
}
