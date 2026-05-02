//! OS-specific automation primitives — keyboard, foreground-window detection,
//! clipboard.
//!
//! On Windows we call into Win32 directly. On other platforms the same
//! functions exist but become no-ops or best-effort, since CS2 only runs on
//! Windows in practice.

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
pub use stub::*;

/// Cross-platform clipboard write. Returns an error if the OS clipboard
/// could not be accessed.
pub fn set_clipboard_text(text: &str) -> anyhow::Result<()> {
    let mut cb = arboard::Clipboard::new()?;
    cb.set_text(text.to_string())?;
    Ok(())
}
