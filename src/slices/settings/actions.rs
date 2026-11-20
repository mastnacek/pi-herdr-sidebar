//! Background `spai reindex` and AI facet classification triggers for S.P.A.I.
use super::classify_service::{classify_note, collect_taxonomy};
use super::state::{AsyncProgress, SettingsState};
use crate::shared::spai_client;
use crate::slices::spai_notes::note::SpaiNoteItem;
use crate::slices::spai_notes::storage_format::format_spai_markdown;
use crate::slices::spai_notes::SpaiNotesState;
use std::sync::mpsc::channel;

/// Fills in the missing translations and vectors of all projects (`spai reindex --all`).
pub fn vectorize_missing_records(settings: &mut SettingsState, notes_state: &mut SpaiNotesState) {
    run_reindex_worker(settings, notes_state);
}

/// `spai` re-embeds only what is missing, so "all" is the same call.
pub fn vectorize_all_records(settings: &mut SettingsState, notes_state: &mut SpaiNotesState) {
    run_reindex_worker(settings, notes_state);
}

fn run_reindex_worker(settings: &mut SettingsState, notes_state: &mut SpaiNotesState) {
    if settings.is_busy {
        return;
    }
    let (tx, rx) = channel();
    settings.task_receiver = Some(rx);
    settings.is_busy = true;
    settings.busy_label = "Doplňuji překlady a vektory (spai reindex)...".to_string();
    settings.busy_step = 0;
    settings.busy_total = 0;
    notes_state.vectors_stale = true;

    let cfg = notes_state.core.clone();
    let cwd = notes_state
        .current_project_path
        .clone()
        .or_else(|| notes_state.projects.first().map(|p| p.path.clone()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    std::thread::spawn(move || {
        let msg = match spai_client::reindex_all(&cfg, &cwd) {
            Ok(summary) => AsyncProgress::Done(format!("✅ {summary}")),
            Err(e) => AsyncProgress::Error(e),
        };
        let _ = tx.send(msg);
    });
}

/// Triggers background 5D facet classification for unclassified records only.
pub fn classify_missing_facets(settings: &mut SettingsState, notes_state: &mut SpaiNotesState) {
    run_classify_worker(settings, notes_state, false);
}

/// Triggers background 5D facet classification for all records (force re-classify all).
pub fn classify_all_facets(settings: &mut SettingsState, notes_state: &mut SpaiNotesState) {
    run_classify_worker(settings, notes_state, true);
}

fn run_classify_worker(
    settings: &mut SettingsState,
    notes_state: &mut SpaiNotesState,
    force_all: bool,
) {
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
    settings.busy_label = if force_all {
        "Překlasifikace všech 5D facetů...".to_string()
    } else {
        "Doplnění chybějících 5D facetů...".to_string()
    };
    settings.busy_step = 0;
    settings.busy_total = 0;

    let api_key = settings.api_key.clone();
    let model = settings.chat_model.clone();
    let taxonomy = collect_taxonomy(&notes_state.projects);

    let mut notes_to_classify: Vec<SpaiNoteItem> = Vec::new();
    for p in &mut notes_state.projects {
        p.ensure_items();
        for item in &p.items {
            if force_all || item.facets.area.is_none() || item.facets.effort.is_none() {
                notes_to_classify.push(item.clone());
            }
        }
    }

    std::thread::spawn(move || {
        let total = notes_to_classify.len();
        if total == 0 {
            let _ = tx.send(AsyncProgress::Done("Všechny záznamy již mají 5D facety".to_string()));
            return;
        }

        let mut processed = 0;
        for mut item in notes_to_classify {
            let _ = tx.send(AsyncProgress::Progress {
                step: processed,
                total,
                label: format!("Klasifikuji: {} ({}/{})", item.id, processed + 1, total),
            });

            match classify_note(&api_key, &model, &item, Some(&taxonomy)) {
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
                    if let Some(dl) = facets.deadline {
                        if item.facets.deadline.is_none() {
                            item.facets.deadline = Some(dl);
                        }
                    }
                    if let Some(prio) = facets.priority {
                        if item.facets.priority.is_none() {
                            item.facets.priority = Some(prio);
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
            "✅ 5D AI Klasifikace dokončena: {}/{} záznamů zatříděno modelem {}",
            processed, total, model
        )));
    });
}
