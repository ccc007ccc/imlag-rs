//! Application-level UI events. The engine drops these on a channel so the
//! GUI can show toasts / status without sharing a parking_lot mutex.

use std::time::SystemTime;

/// One status-bar / toast item produced by the engine.
#[derive(Debug, Clone)]
pub struct UiEvent {
    /// When the event was produced.
    pub at: SystemTime,
    /// Severity level — drives icon / colour in the UI.
    pub level: UiLevel,
    /// Human-readable text (already localised).
    pub message: String,
    /// Tag the GUI can use to deduplicate / route the event.
    pub kind: UiKind,
}

/// Severity of a [`UiEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLevel {
    /// Info / status update.
    Info,
    /// Warning — recoverable problem.
    Warn,
    /// Error — operation failed.
    Error,
}

/// Domain category of a [`UiEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiKind {
    /// GSI listener lifecycle / connectivity.
    Gsi,
    /// Player death observed.
    PlayerDeath,
    /// Chat message sent.
    ChatSent,
    /// CFG mode (file generation, autoexec patching).
    Cfg,
    /// Corpus changed (added / removed / imported).
    Corpus,
    /// Configuration changed.
    Config,
    /// Generic / unknown.
    Other,
}

impl UiEvent {
    /// Create an info-level event.
    pub fn info(kind: UiKind, message: impl Into<String>) -> Self {
        Self {
            at: SystemTime::now(),
            level: UiLevel::Info,
            kind,
            message: message.into(),
        }
    }
    /// Create a warn-level event.
    pub fn warn(kind: UiKind, message: impl Into<String>) -> Self {
        Self {
            at: SystemTime::now(),
            level: UiLevel::Warn,
            kind,
            message: message.into(),
        }
    }
    /// Create an error-level event.
    pub fn error(kind: UiKind, message: impl Into<String>) -> Self {
        Self {
            at: SystemTime::now(),
            level: UiLevel::Error,
            kind,
            message: message.into(),
        }
    }
}
