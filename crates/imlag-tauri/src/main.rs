//! ImLag desktop entry — Tauri 2.
//!
//! The window itself is owned by Tauri (decorations off, transparent on,
//! `windowEffects: ["acrylic"]` declared in `tauri.conf.json`). All
//! Win11 visual effects are produced by the OS compositor on a transparent
//! backbuffer; no `window-vibrancy` or DWM calls are needed.
//!
//! The Rust side keeps the [`imlag_core::Engine`] in [`AppState`] and
//! exposes its capabilities to the React webview via `#[tauri::command]`.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod commands;
mod events;
mod state;

use state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "imlag=info,imlag_tauri=info,cs2_gsi=info".into()),
        )
        .init();

    let data_dir = imlag_core::default_data_dir();
    tracing::info!("data dir = {}", data_dir.display());
    let engine = imlag_core::Engine::bootstrap(data_dir);

    if engine.config_snapshot().auto_start_gsi {
        let engine_for_start = engine.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = engine_for_start.start_gsi().await {
                tracing::error!("auto-start GSI failed: {e}");
            }
        });
    }

    let app_state = AppState::new(engine);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(|app| {
            events::install(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::update_config,
            commands::start_gsi,
            commands::stop_gsi,
            commands::is_gsi_running,
            commands::corpus_list,
            commands::corpus_add,
            commands::corpus_remove,
            commands::corpus_export,
            commands::corpus_import,
            commands::cfg_generate,
            commands::cfg_remove,
            commands::set_language,
            commands::stats_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ImLag");
}
