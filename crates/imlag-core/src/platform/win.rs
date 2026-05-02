//! Windows implementations of keyboard automation and foreground-window
//! detection. Direct Win32 calls via the `windows` crate.
//!
//! Keystroke injection goes through **`SendInput` with scan codes**
//! (`KEYEVENTF_SCANCODE`). CS2 / Source 2 uses SDL2, which on Windows
//! reads raw input — scan-code injection lands directly in SDL's
//! raw-input pipeline. There is no fallback: if SendInput is blocked
//! (UIPI / integrity-level mismatch with CS2) we surface the error so
//! the caller can show it to the user instead of silently looking like
//! it worked. Run imlag with the same elevation as CS2 to make this go
//! away.

#![allow(unsafe_code)]

use std::io;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, HMODULE, HWND};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardState, GetLastInputInfo, MapVirtualKeyW, SendInput, VkKeyScanW, INPUT, INPUT_0,
    INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
    LASTINPUTINFO, MAPVK_VK_TO_VSC, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// CS2's chat box has a small grace period between gaining focus and
/// being ready to accept characters; sub-15ms gaps before paste / Enter
/// can drop the keystroke entirely. This is the floor every press_*
/// helper enforces, regardless of the user's configured `key_delay`.
const MIN_KEY_INTERVAL: Duration = Duration::from_millis(15);

fn at_least(d: Duration) -> Duration {
    if d < MIN_KEY_INTERVAL {
        MIN_KEY_INTERVAL
    } else {
        d
    }
}

const VK_CONTROL: u8 = 0x11;
const VK_RETURN: u8 = 0x0D;
const VK_DELETE: u8 = 0x2E;
const VK_A: u8 = 0x41;
const VK_V: u8 = 0x56;

/// Map a single ASCII char to a Windows virtual-key code.
fn char_to_vk(ch: char) -> u8 {
    match ch as u32 {
        0x0D => VK_RETURN,
        0x11 => VK_CONTROL,
        0x2E => VK_DELETE,
        0x41 => VK_A,
        0x56 => VK_V,
        _ => unsafe {
            let raw = VkKeyScanW(ch as u16);
            // Low byte = vk, high byte = shift state. -1 (0xFFFF) → fallback.
            if raw == -1 {
                ch.to_ascii_uppercase() as u8
            } else {
                (raw & 0xFF) as u8
            }
        },
    }
}

/// Resolve a CS2-style key spec to a Windows virtual-key code.
///
/// Accepts:
///  - Single ASCII characters (`"a"`, `"k"`, `"7"`) → routed through
///    `char_to_vk`.
///  - Named keys, case-insensitive: `ins`, `home`, `end`, `del`, `pgup`,
///    `pgdn`, `up`, `down`, `left`, `right`, `space`, `tab`, `enter`,
///    `backspace`, `f1`..`f24`.
///
/// Returns `None` for unknown specs so callers can fall back gracefully.
pub fn spec_to_vk(spec: &str) -> Option<u8> {
    let s = spec.trim();
    if s.is_empty() {
        return None;
    }
    if s.chars().count() == 1 {
        let ch = s.chars().next().unwrap();
        if ch.is_ascii_alphanumeric() {
            return Some(char_to_vk(ch));
        }
    }
    let lower = s.to_ascii_lowercase();
    let vk: u8 = match lower.as_str() {
        "ins" | "insert" => 0x2D,
        "home" => 0x24,
        "end" => 0x23,
        "del" | "delete" => 0x2E,
        "pgup" | "pageup" => 0x21,
        "pgdn" | "pgdown" | "pagedown" => 0x22,
        "up" => 0x26,
        "down" => 0x28,
        "left" => 0x25,
        "right" => 0x27,
        "space" => 0x20,
        "tab" => 0x09,
        "enter" | "return" => 0x0D,
        "backspace" | "bksp" => 0x08,
        "esc" | "escape" => 0x1B,
        s if s.starts_with('f') => {
            let n: u8 = s[1..].parse().ok()?;
            if (1..=24).contains(&n) {
                0x70 + (n - 1)
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(vk)
}

/// VKs that need the `KEYEVENTF_EXTENDEDKEY` flag — the navigation
/// cluster, arrows, and the right-side modifier keys. Without this flag
/// CS2 occasionally interprets Insert as Numpad-0, etc.
fn is_extended_vk(vk: u8) -> bool {
    matches!(
        vk,
        0x21 // PgUp
        | 0x22 // PgDn
        | 0x23 // End
        | 0x24 // Home
        | 0x25 // Left
        | 0x26 // Up
        | 0x27 // Right
        | 0x28 // Down
        | 0x2D // Insert
        | 0x2E // Delete
        | 0x6F // Numpad Divide
        | 0x90 // NumLock
        | 0xA3 // Right Ctrl
        | 0xA5 // Right Alt
    )
}

/// Build one `INPUT` keyboard event for the given vk and direction.
fn build_input(vk: u8, up: bool) -> INPUT {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
    let mut flags = KEYEVENTF_SCANCODE;
    if is_extended_vk(vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                // wVk MUST be 0 when KEYEVENTF_SCANCODE is set — Windows
                // ignores it and resolves from wScan instead.
                wVk: VIRTUAL_KEY(0),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Push a slice of inputs through `SendInput` atomically. Returns the
/// raw `Win32_Error` (typically 5 = ACCESS_DENIED for UIPI blocks) so
/// callers can decide whether to abort the whole sequence.
fn submit(inputs: &[INPUT]) -> io::Result<()> {
    let n = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if n as usize == inputs.len() {
        return Ok(());
    }
    let err = unsafe { GetLastError() };
    Err(io::Error::from_raw_os_error(err.0 as i32))
}

/// Send one key event (down or up) via `SendInput`.
fn send_key(vk: u8, up: bool) -> io::Result<()> {
    submit(&[build_input(vk, up)])
}

/// Send press + release for a single VK as one `SendInput` batch — the
/// OS delivers both events atomically. Holds the down state for `hold`
/// (≥ [`MIN_KEY_INTERVAL`]) so CS2's input state machine sees a real
/// keystroke, not a glitch.
fn send_key_tap(vk: u8, hold: Duration) -> io::Result<()> {
    submit(&[build_input(vk, false)])?;
    sleep(at_least(hold));
    submit(&[build_input(vk, true)])
}

/// Send a single press-and-release. The `delay` argument controls both the
/// hold time and the rest before returning.
pub fn press_key(ch: char, delay: Duration) -> io::Result<()> {
    let vk = char_to_vk(ch);
    send_key_tap(vk, delay)?;
    sleep(at_least(delay));
    Ok(())
}

/// Like [`press_key`] but accepts a [CS2-style key spec][spec_to_vk]
/// (`"k"`, `"ins"`, `"f5"`, …). Returns `Ok(false)` if the spec is
/// unknown (no key was pressed); `Err` only when injection itself failed.
pub fn press_key_spec(spec: &str, delay: Duration) -> io::Result<bool> {
    let Some(vk) = spec_to_vk(spec) else {
        return Ok(false);
    };
    send_key_tap(vk, delay)?;
    sleep(at_least(delay));
    Ok(true)
}

/// Hold a key down without releasing.
pub fn key_down(ch: char) -> io::Result<()> {
    send_key(char_to_vk(ch), false)
}

/// Release a previously held key.
pub fn key_up(ch: char) -> io::Result<()> {
    send_key(char_to_vk(ch), true)
}

/// Release WASD/Space/Shift/Ctrl/Alt — the keyboard movement set the
/// player is most likely to be holding when chat opens.
///
/// Mouse buttons (vk 0x01/0x02) are intentionally **not** in this list:
/// they're not keyboard VKs, and `SendInput INPUT_KEYBOARD` rejects them.
/// Held mouse buttons don't bleed into the chat box, so we don't need
/// to release them here.
///
/// Errors injecting any single key are logged and ignored — this is a
/// best-effort cleanup, not a load-bearing operation.
pub fn release_movement_keys() {
    const KEYS: &[u8] = &[
        0x57, // W
        0x41, // A
        0x53, // S
        0x44, // D
        0x20, // Space
        0x10, // Shift
        0x11, // Ctrl
        0x12, // Alt
    ];
    for k in KEYS {
        if let Err(e) = send_key(*k, true) {
            tracing::trace!("release_movement_keys: vk=0x{k:02X} keyup failed: {e}");
        }
    }
    sleep(Duration::from_millis(30));
}

/// Release every key the OS currently considers pressed.
///
/// Walks the 256-entry keyboard state from `GetKeyboardState`, sends a
/// `KEYUP` for each VK whose high bit is set. Skips non-keyboard VKs
/// (0x01..=0x06 mouse + reserved, 0xE7 VK_PACKET) and tolerates
/// per-key injection errors silently.
pub fn release_all_keys() {
    let mut state = [0u8; 256];
    if unsafe { GetKeyboardState(&mut state) }.is_err() {
        release_movement_keys();
        return;
    }
    let mut released = 0u32;
    for vk in 1u16..=254u16 {
        if matches!(vk, 0x01..=0x06 | 0xE7) {
            continue;
        }
        if state[vk as usize] & 0x80 != 0 {
            if let Err(e) = send_key(vk as u8, true) {
                tracing::trace!("release_all_keys: vk=0x{vk:02X} keyup failed: {e}");
                continue;
            }
            released += 1;
        }
    }
    if released > 0 {
        sleep(Duration::from_millis(20));
    }
}

/// Type the standard "select all + delete" sequence into the focused control.
pub fn clear_input(delay: Duration) -> io::Result<()> {
    let delay = at_least(delay);
    send_key(VK_CONTROL, false)?;
    sleep(MIN_KEY_INTERVAL);
    send_key_tap(VK_A, delay)?;
    send_key(VK_CONTROL, true)?;
    sleep(MIN_KEY_INTERVAL);
    send_key_tap(VK_DELETE, delay)
}

/// Type the standard Ctrl+V paste shortcut.
pub fn paste_clipboard() -> io::Result<()> {
    send_key(VK_CONTROL, false)?;
    sleep(MIN_KEY_INTERVAL);
    send_key_tap(VK_V, MIN_KEY_INTERVAL)?;
    send_key(VK_CONTROL, true)?;
    // Without this trailing rest CS2 occasionally treats the next
    // synthesised key (Enter) as a Ctrl-modified one before the OS has
    // delivered the modifier-up event.
    sleep(MIN_KEY_INTERVAL);
    Ok(())
}

/// Press the Enter key once.
pub fn press_enter() -> io::Result<()> {
    send_key_tap(VK_RETURN, MIN_KEY_INTERVAL)
}

/// Read the clipboard's current text contents, if any.
///
/// Returns `None` if the clipboard contains non-text data (image, file
/// list, …) — the caller should treat that as "nothing to restore" and
/// leave the new payload in place after sending. Saves/restores are
/// best-effort: a failure here is never fatal.
pub fn clipboard_text() -> Option<String> {
    let mut cb = arboard::Clipboard::new().ok()?;
    cb.get_text().ok()
}

/// Milliseconds since the user's last keyboard or mouse input.
///
/// Wraps `GetLastInputInfo`. Returns `0` on the first failed call so
/// callers fall back to "no idle window" rather than blocking forever.
pub fn idle_millis() -> u32 {
    let mut lii = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    if !unsafe { GetLastInputInfo(&mut lii) }.as_bool() {
        return 0;
    }
    let now = unsafe { GetTickCount() };
    now.saturating_sub(lii.dwTime)
}

/// Returns `true` when the foreground window belongs to the `cs2.exe`
/// process. Returns `false` on any error (insufficient permissions,
/// no foreground window, etc.).
pub fn is_cs2_active() -> bool {
    let hwnd: HWND = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return false;
    }
    let mut pid: u32 = 0;
    let tid = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if tid == 0 {
        return false;
    }
    process_image_name(pid)
        .map(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("cs2.exe"))
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn process_image_name(pid: u32) -> Option<PathBuf> {
    let handle: HANDLE =
        unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()? };
    let mut buf = vec![0u16; 32 * 1024];
    let len = unsafe { GetModuleFileNameExW(handle, HMODULE::default(), &mut buf) };
    let _ = unsafe { CloseHandle(handle) };
    if len == 0 {
        let _ = unsafe { GetLastError() };
        return None;
    }
    buf.truncate(len as usize);
    Some(PathBuf::from(String::from_utf16_lossy(&buf)))
}

// Suppress unused-imports warning from PWSTR if Windows API surface changes.
#[allow(dead_code)]
fn _force_link(_: PWSTR, _: VIRTUAL_KEY) {}
