//! Best-effort no-op implementations for non-Windows targets. CS2 ships only
//! on Windows so the keyboard / window helpers have nothing meaningful to do
//! here — they just log a warning and succeed.

use std::time::Duration;

/// Stub: log a warning, do nothing.
pub fn press_key(_ch: char, _delay: Duration) {
    tracing::warn!("press_key is a no-op on this platform");
}

/// Stub.
pub fn key_down(_ch: char) {}

/// Stub.
pub fn key_up(_ch: char) {}

/// Stub.
pub fn release_movement_keys() {}

/// Stub.
pub fn clear_input(_delay: Duration) {}

/// Stub.
pub fn paste_clipboard() {}

/// Stub.
pub fn press_enter() {}

/// Always returns `false` outside Windows since CS2 cannot be the active
/// window.
pub fn is_cs2_active() -> bool {
    false
}
