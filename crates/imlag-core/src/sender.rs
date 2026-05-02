//! Chat-message sender — drives CS2's chat box by posting Win32 messages
//! directly to its main window.
//!
//! No clipboard, no global SendInput, no `release_all_keys`. We open the
//! chat box with a single key tap, then dribble the message in via
//! `WM_CHAR` per Unicode codepoint, then tap Enter. PostMessage delivery
//! bypasses `BlockInput` entirely, so the dispatch isn't subject to the
//! transient input freezes anti-cheat / overlay layers impose.

use crate::config::AppConfig;
use crate::platform;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Errors produced while sending a chat message.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SendError {
    /// CS2 isn't running / its main window can't be found.
    #[error("CS2 main window not found")]
    NotForeground,
    /// `PostMessageW` rejected one of our messages — extremely rare,
    /// usually means the window handle was destroyed mid-dispatch.
    #[error("key injection failed: {0}")]
    Inject(String),
    /// `tokio::task::spawn_blocking` could not be joined.
    #[error("blocking task join error: {0}")]
    Join(String),
}

/// Hard floor between simulated keystrokes — enough for SDL to observe
/// each event before the next one lands.
const MIN_STEP: Duration = Duration::from_millis(20);

/// Wait for the user to be idle this long before we start typing. With
/// PostMessage there's no OS-level collision, but two sources writing
/// into the same chat box still produces gibberish.
const IDLE_THRESHOLD: Duration = Duration::from_millis(120);
const IDLE_WAIT_BUDGET: Duration = Duration::from_millis(800);
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// One global lock around every dispatch so two near-simultaneous deaths
/// can't interleave their key sequences in CS2's message queue.
static SEND_LOCK: Mutex<()> = Mutex::new(());

/// Sends ImLag's chat messages by posting messages directly to CS2.
#[derive(Clone)]
pub struct ChatMessageSender {
    config: Arc<parking_lot::RwLock<AppConfig>>,
}

impl ChatMessageSender {
    /// Create a sender driven by the supplied shared config.
    pub fn new(config: Arc<parking_lot::RwLock<AppConfig>>) -> Self {
        Self { config }
    }

    /// Synchronously inject a chat message — opens the chat box, types
    /// each character, then taps Enter.
    pub fn send_message_blocking(&self, message: &str) -> Result<(), SendError> {
        // Serialise the entire dispatch.
        let _serialised = SEND_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let snap = self.config.read().clone();
        if !snap.skip_window_check && !platform::is_cs2_active() {
            return Err(SendError::NotForeground);
        }

        // Resolve CS2's HWND once; reuse it for every PostMessage.
        let target = platform::find_cs2_window().ok_or(SendError::NotForeground)?;

        // Don't fight the user's real keyboard activity.
        wait_for_idle(IDLE_THRESHOLD, IDLE_WAIT_BUDGET);

        let key_delay = step_delay(snap.key_delay);
        let chat_key = first_char_or(&snap.chat_key, 'y');

        // Open the chat box — sometimes one tap; force_mode taps three
        // to be doubly sure focus lands.
        let opens = if snap.force_mode { 3 } else { 1 };
        for _ in 0..opens {
            platform::post_char_key(target, chat_key, key_delay).map_err(inject_err)?;
            std::thread::sleep(MIN_STEP);
        }
        // Give SDL a beat to observe SDL_KEYDOWN, route it to chat
        // bind, and switch input focus before we start sending text.
        std::thread::sleep(Duration::from_millis(80));

        // Walk the message char-by-char as WM_CHAR. Surrogate handling
        // lives in platform::post_char.
        for c in message.chars() {
            platform::post_char(target, c).map_err(inject_err)?;
            std::thread::sleep(MIN_STEP);
        }

        // Brief pause before Enter so the last WM_CHAR has time to be
        // appended to the chat buffer.
        std::thread::sleep(MIN_STEP);
        platform::post_enter(target).map_err(inject_err)?;
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

/// Block the calling (blocking) thread until the user has been idle for
/// `threshold`, or `budget` has elapsed.
fn wait_for_idle(threshold: Duration, budget: Duration) {
    let started = Instant::now();
    while started.elapsed() < budget {
        let idle = Duration::from_millis(platform::idle_millis() as u64);
        if idle >= threshold {
            return;
        }
        std::thread::sleep(IDLE_POLL_INTERVAL);
    }
}

fn step_delay(configured_ms: u32) -> Duration {
    let raw = Duration::from_millis(configured_ms as u64);
    if raw < MIN_STEP {
        MIN_STEP
    } else {
        raw
    }
}

fn first_char_or(s: &str, fallback: char) -> char {
    s.chars().next().unwrap_or(fallback)
}

fn inject_err(e: std::io::Error) -> SendError {
    SendError::Inject(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_delay_floors_at_min_step() {
        assert_eq!(step_delay(0), MIN_STEP);
        assert_eq!(step_delay(5), MIN_STEP);
        assert_eq!(step_delay(20), MIN_STEP);
        assert_eq!(step_delay(100), Duration::from_millis(100));
    }

    #[test]
    fn first_char_falls_back_when_empty() {
        assert_eq!(first_char_or("", 'y'), 'y');
        assert_eq!(first_char_or("u", 'y'), 'u');
    }
}
