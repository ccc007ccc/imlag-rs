//! Core logic of ImLag — corpus management, configuration, CS2 cfg
//! generator, GSI integration and chat-message sending.
//!
//! Most callers want [`Engine`] — it bootstraps every sub-system from a
//! single `data_dir` and exposes a [`tokio::sync::broadcast`] channel of
//! [`UiEvent`]s for the GUI to subscribe to.

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod cfg_manager;
pub mod chat;
pub mod config;
pub mod engine;
pub mod events;
pub mod i18n;
pub mod platform;
pub mod sender;

pub use cfg_manager::{CfgError, CfgManager};
pub use chat::{ChatMessageManager, ImportResult};
pub use config::AppConfig;
pub use engine::{Engine, DEFAULT_PORT, GSI_SERVICE_NAME};
pub use events::{UiEvent, UiKind, UiLevel};
pub use sender::{ChatMessageSender, SendError};

/// Resolve the directory where ImLag stores `Config.json` / `Messages.txt`.
///
/// Order:
///  1. The current working directory if it already contains either file
///     (so the original Godot install layout keeps working).
///  2. `<config-dir>/imlag` (e.g. `%APPDATA%\imlag` on Windows).
///  3. The current working directory as a last resort.
pub fn default_data_dir() -> std::path::PathBuf {
    use std::path::PathBuf;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("Config.json").is_file() || cwd.join("Messages.txt").is_file() {
        return cwd;
    }
    if let Some(dirs) = directories::ProjectDirs::from("dev", "ccc007ccc", "imlag") {
        return dirs.config_dir().to_path_buf();
    }
    cwd
}
