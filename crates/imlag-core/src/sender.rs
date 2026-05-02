//! Chat-message sender — drives the keyboard to type a message into the CS2
//! chat box.
//!
//! All work happens on a blocking thread so the synchronous `keybd_event`
//! sleeps don't tie up a tokio worker. Call from async code with
//! [`send_message`](ChatMessageSender::send_message); from sync code use
//! [`send_message_blocking`](ChatMessageSender::send_message_blocking).

use crate::config::AppConfig;
use crate::platform;
use std::sync::Arc;
use std::time::Duration;

/// Errors produced while sending a chat message.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SendError {
    /// The CS2 window check failed.
    #[error("CS2 window not found in foreground")]
    NotForeground,
    /// Clipboard could not be written.
    #[error("clipboard write failed: {0}")]
    Clipboard(String),
    /// `tokio::task::spawn_blocking` could not be joined.
    #[error("blocking task join error: {0}")]
    Join(String),
}

/// Sends ImLag's chat messages by simulating keystrokes.
#[derive(Clone)]
pub struct ChatMessageSender {
    config: Arc<parking_lot::RwLock<AppConfig>>,
}

impl ChatMessageSender {
    /// Create a sender driven by the supplied shared config.
    pub fn new(config: Arc<parking_lot::RwLock<AppConfig>>) -> Self {
        Self { config }
    }

    /// Synchronously inject a chat message — opens the chat box, pastes,
    /// presses enter. Blocks the calling thread for `key_delay * ~7` ms.
    pub fn send_message_blocking(&self, message: &str) -> Result<(), SendError> {
        let snap = self.config.read().clone();
        if !snap.skip_window_check && !platform::is_cs2_active() {
            return Err(SendError::NotForeground);
        }
        platform::release_movement_keys();
        std::thread::sleep(Duration::from_millis(100));

        platform::set_clipboard_text(message).map_err(|e| SendError::Clipboard(e.to_string()))?;
        std::thread::sleep(Duration::from_millis(50));

        let key_delay = Duration::from_millis(snap.key_delay as u64);
        let chat_key = first_char_or(&snap.chat_key, 'y');
        let opens = if snap.force_mode { 3 } else { 1 };
        for _ in 0..opens {
            platform::press_key(chat_key, key_delay);
        }
        std::thread::sleep(key_delay * 2);

        platform::clear_input(key_delay);
        std::thread::sleep(key_delay);
        platform::paste_clipboard();
        std::thread::sleep(key_delay);
        platform::press_enter();
        Ok(())
    }

    /// Async wrapper — runs the blocking send on `spawn_blocking`.
    pub async fn send_message(&self, message: impl Into<String>) -> Result<(), SendError> {
        let me = self.clone();
        let owned = message.into();
        tokio::task::spawn_blocking(move || me.send_message_blocking(&owned))
            .await
            .map_err(|e| SendError::Join(e.to_string()))?
    }
}

fn first_char_or(s: &str, fallback: char) -> char {
    s.chars().next().unwrap_or(fallback)
}
