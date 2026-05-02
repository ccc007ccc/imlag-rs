//! OS-specific automation primitives — keyboard injection via
//! `PostMessageW` to CS2's main window, plus foreground-window detection.
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
