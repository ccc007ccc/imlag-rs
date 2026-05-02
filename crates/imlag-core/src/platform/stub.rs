//! Best-effort no-op implementations for non-Windows targets. CS2 ships only
//! on Windows so the keyboard / window helpers have nothing meaningful to do
//! here — they just log a warning and succeed.

use std::time::Duration;

/// Stub: log a warning, do nothing.
pub fn press_key(_ch: char, _delay: Duration) {
    tracing::warn!("press_key is a no-op on this platform");
}

/// Stub.
pub fn press_key_spec(_spec: &str, _delay: Duration) -> bool {
    tracing::warn!("press_key_spec is a no-op on this platform");
    false
}

/// Stub.
pub fn spec_to_vk(_spec: &str) -> Option<u8> {
    None
}

/// Stub.
pub fn key_down(_ch: char) {}

/// Stub.
pub fn key_up(_ch: char) {}

/// Stub.
pub fn release_movement_keys() {}

/// Stub.
pub fn release_all_keys() {}

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

/// Stub: no clipboard access reported (caller will skip restore).
pub fn clipboard_text() -> Option<String> {
    None
}

/// Stub: pretend the user has just been idle forever, so callers don't
/// gate dispatches on idle detection on non-Windows builds.
pub fn idle_millis() -> u32 {
    u32::MAX
}

/// Stub IL placeholder for non-Windows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IntegrityLevel {
    /// Always returned on non-Windows.
    Unknown,
}

impl IntegrityLevel {
    /// Short human-readable label for display.
    pub fn label(self) -> &'static str {
        "n/a"
    }
}

/// Stub: returns [`IntegrityLevel::Unknown`] outside Windows.
pub fn current_process_integrity_level() -> IntegrityLevel {
    IntegrityLevel::Unknown
}
