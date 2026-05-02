//! Windows implementations of keyboard automation and foreground-window
//! detection. Direct Win32 calls via the `windows` crate.
//!
//! Keystroke injection goes through `SendInput` with **scan codes**
//! (`KEYEVENTF_SCANCODE`). CS2 / Source 2 uses SDL2, which on Windows
//! reads raw input. The legacy `keybd_event` API is documented as
//! superseded — and in practice CS2 frequently drops keystrokes injected
//! that way, *and* doesn't cleanly observe modifier-up events, leaving
//! the in-game state stuck (Ctrl held, chat key fizzles, etc.). Switching
//! to `SendInput` + scan codes makes the injection observable to SDL's
//! raw-input path and atomic per call.

#![allow(unsafe_code)]

use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, HMODULE, HWND};
use windows::Win32::Security::{
    GetTokenInformation, TokenIntegrityLevel, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, GetKeyboardState, GetLastInputInfo, MapVirtualKeyW, SendInput, VkKeyScanW, INPUT,
    INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE, LASTINPUTINFO, MAPVK_VK_TO_VSC, VIRTUAL_KEY,
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

/// Tracks whether we've already noisily logged that SendInput is being
/// blocked (typically UIPI: imlag's IL is below the foreground window's).
/// We log once, then keep silently falling back so the warning-spam
/// doesn't drown the rest of the trace.
static UIPI_WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn warn_uipi_once(context: &str) {
    use std::sync::atomic::Ordering;
    if !UIPI_WARNED.swap(true, Ordering::Relaxed) {
        tracing::warn!(
            "SendInput blocked ({context}) — likely UIPI / integrity-level mismatch with CS2. \
             Falling back to keybd_event; further failures will be silent. \
             If injection still fails, run imlag with the same elevation as CS2."
        );
    }
}

/// Old-API fallback for when `SendInput` fails. `keybd_event` ultimately
/// reaches the same kernel path but its UIPI checks behave differently
/// in some configurations and does land where SendInput won't.
fn keybd_event_fallback(vk: u8, scan: u8, extended: bool, up: bool) {
    let mut flags = KEYBD_EVENT_FLAGS(KEYEVENTF_SCANCODE.0);
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    unsafe {
        keybd_event(vk, scan, flags, 0);
    }
}

/// Send one key event (down or up) via `SendInput`, using a scan code
/// so CS2's raw-input pipeline picks it up reliably. Falls back to
/// `keybd_event` if SendInput is blocked.
fn send_key(vk: u8, up: bool) {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
    let extended = is_extended_vk(vk);
    let mut flags = KEYEVENTF_SCANCODE;
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    let input = INPUT {
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
    };
    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    if sent == 0 {
        warn_uipi_once(&format!("vk=0x{vk:02X} up={up}"));
        keybd_event_fallback(vk, scan as u8, extended, up);
    }
}

/// Send press + release for a single VK as one `SendInput` batch — the
/// OS delivers them atomically to whoever is listening. Falls back to
/// `keybd_event` per-event if SendInput is blocked.
fn send_key_tap(vk: u8, hold: Duration) {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
    let extended = is_extended_vk(vk);
    let mut flags_down = KEYEVENTF_SCANCODE;
    let mut flags_up = KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP;
    if extended {
        flags_down |= KEYEVENTF_EXTENDEDKEY;
        flags_up |= KEYEVENTF_EXTENDEDKEY;
    }
    let down = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: scan,
                dwFlags: flags_down,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let up = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: scan,
                dwFlags: flags_up,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let n = unsafe { SendInput(&[down], std::mem::size_of::<INPUT>() as i32) };
    if n == 0 {
        warn_uipi_once(&format!("tap vk=0x{vk:02X} down"));
        keybd_event_fallback(vk, scan as u8, extended, false);
    }
    sleep(at_least(hold));
    let n = unsafe { SendInput(&[up], std::mem::size_of::<INPUT>() as i32) };
    if n == 0 {
        warn_uipi_once(&format!("tap vk=0x{vk:02X} up"));
        keybd_event_fallback(vk, scan as u8, extended, true);
    }
}

/// Send a single press-and-release. The `delay` argument controls both the
/// hold time and the rest before returning. Both are clamped up to
/// [`MIN_KEY_INTERVAL`] so CS2 always sees the keystroke.
pub fn press_key(ch: char, delay: Duration) {
    let vk = char_to_vk(ch);
    send_key_tap(vk, delay);
    sleep(at_least(delay));
}

/// Like [`press_key`] but accepts a [CS2-style key spec][spec_to_vk]
/// (`"k"`, `"ins"`, `"f5"`, …). Returns `false` if the spec is unknown
/// and no key was pressed.
pub fn press_key_spec(spec: &str, delay: Duration) -> bool {
    let Some(vk) = spec_to_vk(spec) else {
        return false;
    };
    send_key_tap(vk, delay);
    sleep(at_least(delay));
    true
}

/// Hold a key down without releasing.
pub fn key_down(ch: char) {
    send_key(char_to_vk(ch), false);
}

/// Release a previously held key.
pub fn key_up(ch: char) {
    send_key(char_to_vk(ch), true);
}

/// Release WASD/Space/Shift/Ctrl/Alt — the keyboard movement set the
/// player is most likely to be holding when chat opens.
///
/// Mouse buttons (vk 0x01/0x02) used to live in this list, but they are
/// not keyboard VKs — `SendInput INPUT_KEYBOARD` rejects them with
/// `ERROR_ACCESS_DENIED` and the press isn't actually injected anyway.
/// Held mouse buttons don't bleed into the chat box, so we don't need
/// to release them here.
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
        send_key(*k, true);
    }
    sleep(Duration::from_millis(30));
}

/// Release every key the OS currently considers pressed.
///
/// Walks the 256-entry keyboard state from `GetKeyboardState`, sends a
/// `KEYUP` for each VK whose high bit is set. Avoids spamming `keyup` for
/// keys the user wasn't holding, so IME / modifier toggle state stays clean.
///
/// Skips VK_PACKET (0xE7) — it isn't a physical key and "releasing" it
/// can produce stray characters in the focused control.
pub fn release_all_keys() {
    let mut state = [0u8; 256];
    if unsafe { GetKeyboardState(&mut state) }.is_err() {
        // Fallback to the smaller well-known set if the syscall fails.
        release_movement_keys();
        return;
    }
    let mut released = 0u32;
    for vk in 1u16..=254u16 {
        // Skip non-keyboard VKs that GetKeyboardState may report:
        //   0x01..=0x06 — mouse buttons + reserved
        //   0xE7        — VK_PACKET (synthetic unicode injection slot)
        if matches!(vk, 0x01..=0x06 | 0xE7) {
            continue;
        }
        if state[vk as usize] & 0x80 != 0 {
            send_key(vk as u8, true);
            released += 1;
        }
    }
    if released > 0 {
        sleep(Duration::from_millis(20));
    }
}

/// Type the standard "select all + delete" sequence into the focused control.
pub fn clear_input(delay: Duration) {
    let delay = at_least(delay);
    send_key(VK_CONTROL, false);
    sleep(MIN_KEY_INTERVAL);
    send_key_tap(VK_A, delay);
    send_key(VK_CONTROL, true);
    sleep(MIN_KEY_INTERVAL);
    send_key_tap(VK_DELETE, delay);
}

/// Type the standard Ctrl+V paste shortcut.
pub fn paste_clipboard() {
    send_key(VK_CONTROL, false);
    sleep(MIN_KEY_INTERVAL);
    send_key_tap(VK_V, MIN_KEY_INTERVAL);
    send_key(VK_CONTROL, true);
    // Without this trailing rest CS2 occasionally treats the next
    // synthesised key (Enter) as a Ctrl-modified one before the OS has
    // delivered the modifier-up event.
    sleep(MIN_KEY_INTERVAL);
}

/// Press the Enter key once.
pub fn press_enter() {
    send_key_tap(VK_RETURN, MIN_KEY_INTERVAL);
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
fn _force_link(_: PWSTR, _: VIRTUAL_KEY, _: KEYBD_EVENT_FLAGS) {}

/// Coarse process integrity level — bucketed the way Windows reports it
/// in the SID. Higher == more privileged.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IntegrityLevel {
    /// AppContainer / sandboxed.
    Untrusted,
    /// Default for most internet-facing apps.
    Low,
    /// Default for most desktop apps including non-elevated CS2.
    Medium,
    /// Elevated via UAC.
    High,
    /// Local System / kernel.
    System,
    /// Couldn't query (rare).
    Unknown,
}

impl IntegrityLevel {
    /// Short human-readable label for display in status bars and logs.
    pub fn label(self) -> &'static str {
        match self {
            IntegrityLevel::Untrusted => "untrusted",
            IntegrityLevel::Low => "low",
            IntegrityLevel::Medium => "medium",
            IntegrityLevel::High => "high (admin)",
            IntegrityLevel::System => "system",
            IntegrityLevel::Unknown => "unknown",
        }
    }
}

/// The well-known integrity-level RIDs (the last sub-authority of an
/// `S-1-16-x` SID). Source: docs.microsoft.com/windows/win32/secauthz/well-known-sids
const SECURITY_MANDATORY_UNTRUSTED_RID: u32 = 0x0000_0000;
const SECURITY_MANDATORY_LOW_RID: u32 = 0x0000_1000;
const SECURITY_MANDATORY_MEDIUM_RID: u32 = 0x0000_2000;
const SECURITY_MANDATORY_HIGH_RID: u32 = 0x0000_3000;
const SECURITY_MANDATORY_SYSTEM_RID: u32 = 0x0000_4000;

/// Return the integrity level of the **current** process. Used to warn
/// the user up-front if SendInput is going to be UIPI-blocked by CS2.
pub fn current_process_integrity_level() -> IntegrityLevel {
    use windows::Win32::Foundation::HANDLE;

    let mut token: HANDLE = HANDLE::default();
    let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_ok() };
    if !opened {
        return IntegrityLevel::Unknown;
    }

    // First query asks for the required buffer size.
    let mut needed: u32 = 0;
    let _ = unsafe { GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut needed) };
    if needed == 0 {
        let _ = unsafe { CloseHandle(token) };
        return IntegrityLevel::Unknown;
    }

    let mut buf = vec![0u8; needed as usize];
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buf.as_mut_ptr() as *mut _),
            needed,
            &mut needed,
        )
        .is_ok()
    };
    let _ = unsafe { CloseHandle(token) };
    if !ok {
        return IntegrityLevel::Unknown;
    }

    // The buffer is a TOKEN_MANDATORY_LABEL whose Label.Sid points at a
    // SID with at least one sub-authority — the last one is the IL RID.
    let label = unsafe { &*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL) };
    let psid = label.Label.Sid;
    if psid.is_invalid() {
        return IntegrityLevel::Unknown;
    }
    // SID layout: Revision (1) | SubAuthorityCount (1) | Identifier (6) | SubAuthority[]
    // We want the last sub-authority. Read manually because windows crate
    // doesn't expose GetSidSubAuthority/GetSidSubAuthorityCount in our profile.
    let raw = psid.0 as *const u8;
    let count = unsafe { *raw.add(1) } as usize;
    if count == 0 {
        return IntegrityLevel::Unknown;
    }
    let last_index = count - 1;
    let sub_auth_ptr = unsafe { raw.add(8 + last_index * 4) as *const u32 };
    let rid = unsafe { sub_auth_ptr.read_unaligned() };

    match rid {
        r if r >= SECURITY_MANDATORY_SYSTEM_RID => IntegrityLevel::System,
        r if r >= SECURITY_MANDATORY_HIGH_RID => IntegrityLevel::High,
        r if r >= SECURITY_MANDATORY_MEDIUM_RID => IntegrityLevel::Medium,
        r if r >= SECURITY_MANDATORY_LOW_RID => IntegrityLevel::Low,
        SECURITY_MANDATORY_UNTRUSTED_RID => IntegrityLevel::Untrusted,
        _ => IntegrityLevel::Unknown,
    }
}
