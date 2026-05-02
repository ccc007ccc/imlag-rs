//! Bridge between the engine's broadcast channel and the Tauri webview.
//!
//! At setup time we subscribe to `Engine::subscribe_ui()` and forward every
//! [`UiEvent`] to the frontend as a serialised payload on the `ui-event`
//! channel. The frontend's `lib/events.ts` has the matching type.

use imlag_core::{UiEvent, UiKind, UiLevel};
use serde::Serialize;
use tauri::{App, Emitter, Manager};
use tokio::sync::broadcast::error::RecvError;

use crate::state::AppState;

/// Wire-format mirror of [`UiEvent`]. Strings instead of enums so that
/// JS code can switch on them directly without an enum codec.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiEventDto {
    /// Unix epoch milliseconds the event was produced.
    pub timestamp_ms: u64,
    /// `"info" | "warn" | "error"`.
    pub level: &'static str,
    /// Domain category — see `UiKind`.
    pub kind: &'static str,
    /// Already-localised human text.
    pub message: String,
}

impl From<UiEvent> for UiEventDto {
    fn from(e: UiEvent) -> Self {
        let timestamp_ms =
            e.at.duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or_default();
        Self {
            timestamp_ms,
            level: match e.level {
                UiLevel::Info => "info",
                UiLevel::Warn => "warn",
                UiLevel::Error => "error",
            },
            kind: match e.kind {
                UiKind::Gsi => "gsi",
                UiKind::PlayerDeath => "playerDeath",
                UiKind::ChatSent => "chatSent",
                UiKind::Cfg => "cfg",
                UiKind::Corpus => "corpus",
                UiKind::Config => "config",
                UiKind::Other => "other",
            },
            message: e.message,
        }
    }
}

/// Spawn the forwarding task. Called once from `main.rs` setup.
pub fn install(app: &mut App) -> tauri::Result<()> {
    let engine = app.state::<AppState>().engine.clone();
    let handle = app.handle().clone();
    let mut rx = engine.subscribe_ui();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(evt) => {
                    let dto = UiEventDto::from(evt);
                    if let Err(e) = handle.emit("ui-event", dto) {
                        tracing::warn!("emit ui-event failed: {e}");
                    }
                }
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!("ui-event listener lagged, dropped {skipped} events");
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
    Ok(())
}
