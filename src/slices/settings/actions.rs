//! Vectorization and AI facet classification triggers for S.P.A.I.
use super::classify_service::classify_project_notes;
use super::state::SettingsState;
use super::vector_service::vectorize_project_items;
use crate::slices::spai_notes::SpaiNotesState;

/// Triggers vectorization / re-embedding of all records across discovered projects using OpenRouter.
pub fn vectorize_all_records(settings: &mut SettingsState, notes_state: &mut SpaiNotesState) {
    if settings.api_key.trim().is_empty() {
        settings.status_message = Some(
            "❌ Chybí OpenRouter API klíč! Nastavte jej v záložce Nastavení (klávesa 6 -> e)"
                .to_string(),
        );
        return;
    }

    settings.vectorizing = true;
    let mut total_vectorized = 0;
    let mut total_records = 0;

    for p in &mut notes_state.projects {
        p.ensure_items();
        total_records += p.items.len();
        match vectorize_project_items(
            &settings.api_key,
            &settings.embedding_model,
            &p.path,
            &p.items,
        ) {
            Ok(count) => total_vectorized += count,
            Err(err) => {
                settings.vectorizing = false;
                settings.status_message = Some(format!("❌ Chyba vektorizace: {}", err));
                return;
            }
        }
    }

    settings.vectorizing = false;
    settings.total_records = total_records;
    settings.vector_count = total_vectorized;
    settings.status_message = Some(format!(
        "✅ Vektorizace dokončena: {} záznamů uloženo do .vectors.json (model: {})",
        total_vectorized, settings.embedding_model
    ));
}

/// Triggers AI 5D facet classification for unclassified records using OpenRouter Chat.
pub fn classify_facets_all(settings: &mut SettingsState, notes_state: &mut SpaiNotesState) {
    if settings.api_key.trim().is_empty() {
        settings.status_message = Some(
            "❌ Chybí OpenRouter API klíč! Nastavte jej v záložce Nastavení (klávesa 6 -> e)"
                .to_string(),
        );
        return;
    }

    let mut total_classified = 0;

    for p in &mut notes_state.projects {
        p.ensure_items();
        match classify_project_notes(
            &settings.api_key,
            &settings.chat_model,
            &mut p.items,
            true,
        ) {
            Ok(count) => total_classified += count,
            Err(err) => {
                settings.status_message = Some(format!("❌ Chyba 5D klasifikace: {}", err));
                return;
            }
        }
    }

    settings.status_message = Some(format!(
        "✅ 5D AI Klasifikace dokončena: {} záznamů zatříděno modelem {}",
        total_classified, settings.chat_model
    ));
}
