//! Chat-message sender — drives the keyboard to type a message into the CS2
//! chat box.
//!
//! All work happens on a blocking thread so the synchronous `SendInput`
//! sleeps don't tie up a tokio worker. Call from async code with
//! [`send_message`](ChatMessageSender::send_message); from sync code use
//! [`send_message_blocking`](ChatMessageSender::send_message_blocking).

use crate::config::AppConfig;
use crate::platform;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Errors produced while sending a chat message.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SendError {
    /// The CS2 window check failed.
    #[error("CS2 window not found in foreground")]
    NotForeground,
    /// Clipboard could not be written or did not stick.
    #[error("clipboard write failed: {0}")]
    Clipboard(String),
    /// `tokio::task::spawn_blocking` could not be joined.
    #[error("blocking task join error: {0}")]
    Join(String),
}

/// Hard floor between simulated keystrokes — matches the platform layer's
/// own minimum and protects against config values that round to zero.
const MIN_STEP: Duration = Duration::from_millis(15);

/// Wait for the user to be idle this long before we start typing. Picks
/// up most natural "I just got shot" finger-mash windows; if the user
/// keeps drumming the keyboard past [`IDLE_WAIT_BUDGET`] we go anyway,
/// because making the dispatch genuinely non-blocking matters more than
/// perfect collision avoidance.
const IDLE_THRESHOLD: Duration = Duration::from_millis(120);
const IDLE_WAIT_BUDGET: Duration = Duration::from_millis(800);
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// How long to wait *after* writing the clipboard before reading it back
/// for verification. arboard's `set_text` returns once the syscall is
/// done but Windows propagates the new contents asynchronously; without
/// this delay verify_clipboard occasionally still sees the previous
/// owner's payload.
const CLIPBOARD_PROPAGATE: Duration = Duration::from_millis(60);
/// Maximum number of `set_text → verify` loops before giving up.
const CLIPBOARD_VERIFY_ATTEMPTS: usize = 4;

/// One global lock around every `send_message_blocking` call.
///
/// Two near-simultaneous deaths used to race on the clipboard: the first
/// dispatch's RAII guard would restore the user's original clipboard
/// contents *while the second dispatch was mid-paste*, and CS2 would
/// happily send whatever was in your clipboard to chat. This mutex keeps
/// the entire snapshot → set → paste → restore window strictly serial.
static SEND_LOCK: Mutex<()> = Mutex::new(());

/// RAII guard that snapshots the clipboard on entry and restores it on
/// drop — even if the typing pipeline panics or returns early.
///
/// **Drop is gated by [`SEND_LOCK`]** held by the surrounding send call,
/// so the restore can't race against a sibling dispatch.
struct ClipboardGuard {
    previous: Option<String>,
}

impl ClipboardGuard {
    fn snapshot() -> Self {
        Self {
            previous: platform::clipboard_text(),
        }
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // Give CS2 a beat to actually paste & submit before we yank the
        // text back. 80ms covers the worst case observed locally.
        std::thread::sleep(Duration::from_millis(80));
        if let Some(prev) = self.previous.take() {
            if let Err(e) = platform::set_clipboard_text(&prev) {
                tracing::warn!("could not restore clipboard: {e}");
            }
        }
    }
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
    /// presses enter. Restores the clipboard on the way out.
    ///
    /// All calls serialise on [`SEND_LOCK`]; if a second send arrives
    /// while one is already running it will queue rather than racing
    /// the clipboard.
    pub fn send_message_blocking(&self, message: &str) -> Result<(), SendError> {
        // Hold this for the entire payload — including the guard's
        // 80ms drain — so two deaths can't race on the clipboard.
        let _serialised = SEND_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let snap = self.config.read().clone();
        if !snap.skip_window_check && !platform::is_cs2_active() {
            return Err(SendError::NotForeground);
        }

        // Wait for the player to stop spazzing on the keyboard so our
        // synthetic strokes don't fight with real ones. Bounded so we
        // don't sit forever if the user is permanently mashing.
        wait_for_idle(IDLE_THRESHOLD, IDLE_WAIT_BUDGET);

        // Anything held down (W/A/S/D, Ctrl, jump, etc.) would otherwise
        // get baked into the chat box.
        platform::release_all_keys();
        std::thread::sleep(Duration::from_millis(40));

        // Snapshot the clipboard before we overwrite it; the guard
        // restores it on drop, so every early return below stays clean.
        let _restore = ClipboardGuard::snapshot();

        // Set + verify in a small loop. arboard's set_text occasionally
        // appears to "succeed" but the new contents are not yet visible
        // to GetClipboardData when CS2 reads it, especially under load.
        // Read it back; if we see something else, retry.
        set_clipboard_verified(message)?;

        let key_delay = step_delay(snap.key_delay);
        let chat_key = first_char_or(&snap.chat_key, 'y');
        let opens = if snap.force_mode { 3 } else { 1 };
        for _ in 0..opens {
            platform::press_key(chat_key, key_delay);
            std::thread::sleep(MIN_STEP);
        }
        // The chat box needs a beat after focus before it accepts
        // input — paste-into-an-unfocused-control is the #1 cause of
        // "the box appears but no text".
        std::thread::sleep(Duration::from_millis(80));

        platform::clear_input(key_delay);
        std::thread::sleep(MIN_STEP);
        platform::paste_clipboard();
        std::thread::sleep(MIN_STEP);
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

/// Write `text` to the clipboard, then read it back to confirm. Retries
/// up to [`CLIPBOARD_VERIFY_ATTEMPTS`] times.
fn set_clipboard_verified(text: &str) -> Result<(), SendError> {
    let mut last_err: Option<String> = None;
    for _ in 0..CLIPBOARD_VERIFY_ATTEMPTS {
        if let Err(e) = platform::set_clipboard_text(text) {
            last_err = Some(e.to_string());
            std::thread::sleep(CLIPBOARD_PROPAGATE);
            continue;
        }
        std::thread::sleep(CLIPBOARD_PROPAGATE);
        match platform::clipboard_text() {
            Some(actual) if actual == text => return Ok(()),
            Some(actual) => {
                last_err = Some(format!(
                    "clipboard mismatch (got {} chars, expected {})",
                    actual.chars().count(),
                    text.chars().count()
                ));
            }
            None => {
                last_err = Some("clipboard read returned None".into());
            }
        }
    }
    Err(SendError::Clipboard(
        last_err.unwrap_or_else(|| "unknown".into()),
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_delay_floors_at_min_step() {
        assert_eq!(step_delay(0), MIN_STEP);
        assert_eq!(step_delay(5), MIN_STEP);
        assert_eq!(step_delay(15), MIN_STEP);
        assert_eq!(step_delay(100), Duration::from_millis(100));
    }

    #[test]
    fn first_char_falls_back_when_empty() {
        assert_eq!(first_char_or("", 'y'), 'y');
        assert_eq!(first_char_or("u", 'y'), 'u');
    }
}
