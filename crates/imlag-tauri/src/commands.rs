//! Tauri command surface — every method here is callable from the
//! React webview through `invoke("name", { ... })`. All errors are
//! surfaced as plain strings so the frontend can display them via
//! its toast system without losing fidelity.

use crate::state::AppState;
use imlag_core::{config::normalize_language, i18n, AppConfig, ImportResult, UiEvent, UiKind};
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

/// Aggregate counters shown in the status bar.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsSummary {
    /// Total entries in the corpus.
    pub corpus_count: usize,
    /// Whether the dispatch cfg + autoexec block are currently installed.
    pub cfg_installed: bool,
}

// ─── Configuration ────────────────────────────────────────────────────

/// Read a snapshot of the current configuration.
#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.engine.config_snapshot()
}

/// Replace the configuration wholesale, normalising and persisting it.
#[tauri::command]
pub fn update_config(
    state: State<'_, AppState>,
    mut config: AppConfig,
) -> Result<AppConfig, String> {
    config.normalize();
    {
        let cfg_arc = state.engine.config();
        let mut guard = cfg_arc.write();
        *guard = config.clone();
    }
    imlag_core::i18n::set_language(config.language.as_str());
    state.engine.save_config().map_err(|e| e.to_string())?;
    Ok(state.engine.config_snapshot())
}

/// Switch UI language and persist it. Accepts any reasonable spelling.
#[tauri::command]
pub fn set_language(state: State<'_, AppState>, language: String) -> Result<String, String> {
    let lang = normalize_language(&language);
    {
        let cfg_arc = state.engine.config();
        let mut guard = cfg_arc.write();
        guard.language = lang.clone();
    }
    imlag_core::i18n::set_language(lang.as_str());
    state.engine.save_config().map_err(|e| e.to_string())?;
    Ok(lang)
}

// ─── GSI lifecycle ────────────────────────────────────────────────────

/// Start the Game-State-Integration listener.
#[tauri::command]
pub async fn start_gsi(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.start_gsi().await.map_err(|e| e.to_string())
}

/// Stop the GSI listener (idempotent).
#[tauri::command]
pub async fn stop_gsi(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.stop_gsi().await.map_err(|e| e.to_string())
}

/// Whether the GSI listener is currently bound to a port.
#[tauri::command]
pub fn is_gsi_running(state: State<'_, AppState>) -> bool {
    state.engine.is_gsi_running()
}

// ─── Corpus ───────────────────────────────────────────────────────────

/// All corpus entries in insertion order.
#[tauri::command]
pub fn corpus_list(state: State<'_, AppState>) -> Vec<String> {
    state.engine.corpus().all()
}

/// Append a new entry; returns `true` when the message was actually added.
#[tauri::command]
pub fn corpus_add(state: State<'_, AppState>, message: String) -> Result<bool, String> {
    let added = state.engine.corpus().add(&message);
    if added {
        state.engine.save_corpus().map_err(|e| e.to_string())?;
        state.engine.emit(UiEvent::info(
            UiKind::Corpus,
            i18n::t_args("status.message_added", [message.as_str()].as_slice()),
        ));
    }
    Ok(added)
}

/// Remove an entry; returns `true` when something was removed.
#[tauri::command]
pub fn corpus_remove(state: State<'_, AppState>, message: String) -> Result<bool, String> {
    let removed = state.engine.corpus().remove(&message);
    if removed {
        state.engine.save_corpus().map_err(|e| e.to_string())?;
        state.engine.emit(UiEvent::info(
            UiKind::Corpus,
            i18n::t_args("status.message_removed", [message.as_str()].as_slice()),
        ));
    }
    Ok(removed)
}

/// Export the corpus to a plaintext file at `path`.
#[tauri::command]
pub fn corpus_export(state: State<'_, AppState>, path: String) -> Result<usize, String> {
    let p = PathBuf::from(path);
    state
        .engine
        .corpus()
        .export_to_file(&p)
        .map_err(|e| e.to_string())
}

/// Import additional corpus entries from a plaintext file at `path`.
#[tauri::command]
pub fn corpus_import(state: State<'_, AppState>, path: String) -> Result<ImportResult, String> {
    let p = PathBuf::from(path);
    let result = state
        .engine
        .corpus()
        .import_from_file(&p)
        .map_err(|e| e.to_string())?;
    if result.added > 0 {
        state.engine.save_corpus().map_err(|e| e.to_string())?;
        state.engine.emit(UiEvent::info(
            UiKind::Corpus,
            i18n::t_args(
                "status.corpus_imported",
                [
                    result.added.to_string().as_str(),
                    result.skipped.to_string().as_str(),
                ]
                .as_slice(),
            ),
        ));
    }
    Ok(result)
}

// ─── CFG mode ─────────────────────────────────────────────────────────

/// Generate (or refresh) the dispatch cfg and the autoexec bind.
#[tauri::command]
pub fn cfg_generate(state: State<'_, AppState>) -> Result<(), String> {
    state
        .engine
        .cfg_manager()
        .install()
        .map_err(|e| e.to_string())
}

/// Remove every previously generated cfg file & autoexec line.
#[tauri::command]
pub fn cfg_remove(state: State<'_, AppState>) -> Result<(), String> {
    state
        .engine
        .cfg_manager()
        .restore()
        .map_err(|e| e.to_string())
}

// ─── Stats ────────────────────────────────────────────────────────────

/// Counters shown in the status bar.
#[tauri::command]
pub fn stats_summary(state: State<'_, AppState>) -> StatsSummary {
    StatsSummary {
        corpus_count: state.engine.corpus().len(),
        cfg_installed: state.engine.cfg_manager().is_installed(),
    }
}
