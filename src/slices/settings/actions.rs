//! Background vectorization and AI facet classification triggers for S.P.A.I.
use super::classify_service::classify_note;
use super::state::{AsyncProgress, SettingsState};
use super::vector_service::vectorize_project_items_with_progress;
use crate::slices::spai_notes::note::SpaiNoteItem;
use crate::slices::spai_notes::storage_format::format_spai_markdown;
use crate::slices::spai_notes::SpaiNotesState;
use std::sync::mpsc::channel;

/// Triggers background vectorization of all records across discovered projects.
pub fn vectorize_all_records(settings: &mut SettingsState, notes_state: &mut SpaiNotesState) {
    if settings.is_busy {
        return;
    }
    if settings.api_key.trim().is_empty() {
        settings.status_message = Some(
            "❌ Chybí OpenRouter API klíč! Nastavte jej v záložce Nastavení (klávesa 6 -> e)"
                .to_string(),
        );
        return;
    }

    let (tx, rx) = channel();
    settings.task_receiver = Some(rx);
    settings.is_busy = true;
    settings.busy_label = "Příprava vektorizace...".to_string();
    settings.busy_step = 0;
    settings.busy_total = 0;

    let api_key = settings.api_key.clone();
    let model = settings.embedding_model.clone();

    let mut projects_data = Vec::new();
    for p in &mut notes_state.projects {
        p.ensure_items();
        projects_data.push((p.path.clone(), p.items.clone()));
    }

    std::thread::spawn(move || {
        let mut total_records = 0;
        for (_, items) in &projects_data {
            total_records += items.len();
        }

        if total_records == 0 {
            let _ = tx.send(AsyncProgress::Done("Žádné záznamy k vektorizaci".to_string()));
            return;
        }

        let _ = tx.send(AsyncProgress::Progress {
            step: 0,
            total: total_records,
            label: format!("Indexuji {} záznamů...", total_records),
        });

        let mut processed = 0;
        for (project_path, items) in projects_data {
            if items.is_empty() {
                continue;
            }
            if let Err(err) = vectorize_project_items_with_progress(
                &api_key,
                &model,
                &project_path,
                &items,
                &tx,
                &mut processed,
                total_records,
            ) {
                let _ = tx.send(AsyncProgress::Error(err));
                return;
            }
        }

        let _ = tx.send(AsyncProgress::Done(format!(
            "✅ Vektorizace dokončena: {}/{} záznamů indexováno modelem {}",
            processed, total_records, model
        )));
    });
}

/// Triggers background 5D facet classification for unclassified records.
pub fn classify_facets_all(settings: &mut SettingsState, notes_state: &mut SpaiNotesState) {
    if settings.is_busy {
        return;
    }
    if settings.api_key.trim().is_empty() {
        settings.status_message = Some(
            "❌ Chybí OpenRouter API klíč! Nastavte jej v záložce Nastavení (klávesa 6 -> e)"
                .to_string(),
        );
        return;
    }

    let (tx, rx) = channel();
    settings.task_receiver = Some(rx);
    settings.is_busy = true;
    settings.busy_label = "Příprava 5D klasifikace...".to_string();
    settings.busy_step = 0;
    settings.busy_total = 0;

    let api_key = settings.api_key.clone();
    let model = settings.chat_model.clone();

    let mut unclassified: Vec<SpaiNoteItem> = Vec::new();
    for p in &mut notes_state.projects {
        p.ensure_items();
        for item in &p.items {
            if item.facets.area.is_none() || item.facets.effort.is_none() {
                unclassified.push(item.clone());
            }
        }
    }

    std::thread::spawn(move || {
        let total = unclassified.len();
        if total == 0 {
            let _ = tx.send(AsyncProgress::Done("Všechny záznamy již mají 5D facety".to_string()));
            return;
        }

        let mut processed = 0;
        for mut item in unclassified {
            let _ = tx.send(AsyncProgress::Progress {
                step: processed,
                total,
                label: format!("Klasifikuji: {} ({}/{})", item.id, processed + 1, total),
            });

            match classify_note(&api_key, &model, &item) {
                Ok(facets) => {
                    if let Some(area) = facets.area {
                        item.facets.area = Some(area);
                    }
                    if let Some(effort) = facets.effort {
                        item.facets.effort = Some(effort);
                    }
                    if let Some(urgency) = facets.urgency {
                        item.facets.urgency = Some(urgency);
                    }
                    if let Some(who) = facets.who {
                        item.facets.who = Some(who);
                    }
                    if let Some(proj) = facets.project {
                        if item.facets.project.is_none() {
                            item.facets.project = Some(proj);
                        }
                    }

                    let content = format_spai_markdown(&item);
                    if let Err(e) = std::fs::write(&item.file_path, content) {
                        let _ = tx.send(AsyncProgress::Error(format!(
                            "Chyba zápisu {}: {}",
                            item.file_path.display(),
                            e
                        )));
                        return;
                    }
                    processed += 1;
                }
                Err(err) => {
                    let _ = tx.send(AsyncProgress::Error(err));
                    return;
                }
            }
        }

        let _ = tx.send(AsyncProgress::Done(format!(
            "✅ 5D AI Klasifikace dokončena: {} záznamů zatříděno modelem {}",
            processed, model
        )));
    });
}
