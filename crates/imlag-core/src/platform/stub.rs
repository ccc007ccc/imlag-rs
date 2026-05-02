//! Best-effort no-op implementations for non-Windows targets. CS2 ships only
//! on Windows so the keyboard / window helpers have nothing meaningful to do
//! here — they just log a warning and succeed.

use std::io;
use std::time::Duration;

/// Stub Cs2Window placeholder.
#[derive(Clone, Copy, Debug)]
pub struct Cs2Window(());

/// Stub: pretend CS2 isn't running.
pub fn find_cs2_window() -> Option<Cs2Window> {
    None
}

/// Stub.
pub fn press_key_spec(_spec: &str, _delay: Duration) -> io::Result<bool> {
    tracing::warn!("press_key_spec is a no-op on this platform");
    Ok(false)
}

/// Stub.
pub fn spec_to_vk(_spec: &str) -> Option<u8> {
    None
}

/// Stub.
pub fn post_char_key(_target: Cs2Window, _ch: char, _hold: Duration) -> io::Result<()> {
    Ok(())
}

/// Stub.
pub fn post_char(_target: Cs2Window, _c: char) -> io::Result<()> {
    Ok(())
}

/// Stub.
pub fn post_enter(_target: Cs2Window) -> io::Result<()> {
    Ok(())
}

/// Always returns `false` outside Windows since CS2 cannot be the active
/// window.
pub fn is_cs2_active() -> bool {
    false
}

/// Stub: pretend the user has just been idle forever, so callers don't
/// gate dispatches on idle detection on non-Windows builds.
pub fn idle_millis() -> u32 {
    u32::MAX
}
