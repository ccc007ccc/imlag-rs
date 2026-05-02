//! Tauri-managed application state — owns a single [`imlag_core::Engine`]
//! handle and exposes it to all `#[tauri::command]` functions.

use imlag_core::Engine;

/// Process-wide handle stored in `tauri::State<AppState>`.
pub struct AppState {
    /// Shared engine. Internally cheap to clone (it wraps Arc).
    pub engine: Engine,
}

impl AppState {
    /// Build a fresh state from a bootstrapped engine.
    pub fn new(engine: Engine) -> Self {
        Self { engine }
    }
}
