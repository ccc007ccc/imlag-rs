//! Windows implementations of keyboard automation and foreground-window
//! detection — direct Win32 calls via the `windows` crate.
//!
//! Keystrokes are delivered with **`PostMessageW`** straight to CS2's
//! main window. This deliberately *bypasses* the OS global input queue:
//!
//!  - SendInput / keybd_event are subject to `BlockInput`, which CS2
//!    and various anti-cheat / overlay layers toggle for hundreds of
//!    milliseconds at a time during loads, menus, and AC sweeps. That
//!    surface returns ERROR_ACCESS_DENIED (5) and the press is lost.
//!  - PostMessage delivers WM_KEYDOWN/UP/CHAR directly to the target
//!    window's message queue — BlockInput does not affect it.
//!
//! The trade-off is that we now depend on SDL2 reading keyboard input
//! through `WindowProc` (which it does by default). Games that opt into
//! `SDL_HINT_WINDOWS_RAW_KEYBOARD` would not see these events; CS2 in
//! practice does respond to WindowProc messages.

#![allow(unsafe_code)]

use std::io;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use windows::core::PWSTR;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, BOOL, HANDLE, HMODULE, HWND, LPARAM, WPARAM,
};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetLastInputInfo, MapVirtualKeyW, VkKeyScanW, LASTINPUTINFO, MAPVK_VK_TO_VSC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowThreadProcessId,
    IsWindowVisible, PostMessageW, WM_CHAR, WM_KEYDOWN, WM_KEYUP,
};

/// Floor between simulated keystrokes — enough for SDL's per-frame
/// poll to observe each event. PostMessage delivery itself is
/// instantaneous, but giving each keystroke a tick gives SDL_KEYDOWN
/// time to propagate through the chat-input handler before the next
/// event lands.
const MIN_KEY_INTERVAL: Duration = Duration::from_millis(20);

fn at_least(d: Duration) -> Duration {
    if d < MIN_KEY_INTERVAL {
        MIN_KEY_INTERVAL
    } else {
        d
    }
}

const VK_RETURN: u8 = 0x0D;

/// Map a single ASCII char to a Windows virtual-key code.
fn char_to_vk(ch: char) -> u8 {
    if ch == '\r' {
        return VK_RETURN;
    }
    unsafe {
        let raw = VkKeyScanW(ch as u16);
        // Low byte = vk, high byte = shift state. -1 (0xFFFF) → fallback.
        if raw == -1 {
            ch.to_ascii_uppercase() as u8
        } else {
            (raw & 0xFF) as u8
        }
    }
}

/// Resolve a CS2-style key spec to a Windows virtual-key code.
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

/// VKs that need the extended-key bit (lparam bit 24) set when posted.
fn is_extended_vk(vk: u8) -> bool {
    matches!(
        vk,
        0x21 | 0x22
            | 0x23
            | 0x24
            | 0x25
            | 0x26
            | 0x27
            | 0x28
            | 0x2D
            | 0x2E
            | 0x6F
            | 0x90
            | 0xA3
            | 0xA5
    )
}

/// Build the `lparam` for `WM_KEYDOWN` / `WM_KEYUP`. Layout per MSDN:
///   bits 0–15  repeat count (always 1)
///   bits 16–23 scan code
///   bit 24     extended key flag
///   bit 29     context (alt) — always 0 for non-alt keys
///   bit 30     previous key state (1 = was already down)
///   bit 31     transition (1 = being released)
fn build_lparam(scan: u8, extended: bool, up: bool) -> LPARAM {
    let mut lp: u32 = 1; // repeat count = 1
    lp |= (scan as u32) << 16;
    if extended {
        lp |= 1 << 24;
    }
    if up {
        lp |= 1 << 30; // previous-down
        lp |= 1 << 31; // transition (release)
    }
    LPARAM(lp as isize)
}

/// Owned reference to a CS2 top-level HWND. Returned by
/// [`find_cs2_window`] and consumed by the post_* helpers. The wrapper
/// keeps the unsafe `HWND` out of upper layers.
#[derive(Clone, Copy, Debug)]
pub struct Cs2Window(isize);

impl Cs2Window {
    fn hwnd(self) -> HWND {
        HWND(self.0 as *mut _)
    }
}

struct EnumState {
    found: Option<HWND>,
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lp: LPARAM) -> BOOL {
    let state = unsafe { &mut *(lp.0 as *mut EnumState) };
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return BOOL(1);
    }
    // Skip windows with no title — CS2's main game window has one,
    // helper / console windows often do not.
    if unsafe { GetWindowTextLengthW(hwnd) } == 0 {
        return BOOL(1);
    }
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return BOOL(1);
    }
    if let Some(p) = process_image_name(pid) {
        let is_cs2 = p
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("cs2.exe"))
            .unwrap_or(false);
        if is_cs2 {
            state.found = Some(hwnd);
            return BOOL(0); // stop enumeration
        }
    }
    BOOL(1)
}

/// Locate CS2's main window. Tries the foreground window first (cheap
/// and the normal case while the user is in-game) and falls back to a
/// full top-level enumeration so dispatches still work when the user
/// has alt-tabbed away with `skip_window_check` enabled.
pub fn find_cs2_window() -> Option<Cs2Window> {
    // Fast path: foreground.
    let fg = unsafe { GetForegroundWindow() };
    if !fg.0.is_null() {
        let mut pid: u32 = 0;
        unsafe { GetWindowThreadProcessId(fg, Some(&mut pid)) };
        if pid != 0 {
            if let Some(p) = process_image_name(pid) {
                let is_cs2 = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("cs2.exe"))
                    .unwrap_or(false);
                if is_cs2 {
                    return Some(Cs2Window(fg.0 as isize));
                }
            }
        }
    }

    // Fallback: enumerate top-level windows.
    let mut state = EnumState { found: None };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut state as *mut _ as isize));
    }
    state.found.map(|h| Cs2Window(h.0 as isize))
}

/// Post one keystroke event (down or up) to CS2.
fn post_key_event(target: Cs2Window, vk: u8, up: bool) -> io::Result<()> {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u8;
    let extended = is_extended_vk(vk);
    let lp = build_lparam(scan, extended, up);
    let msg = if up { WM_KEYUP } else { WM_KEYDOWN };
    let r = unsafe { PostMessageW(target.hwnd(), msg, WPARAM(vk as usize), lp) };
    if r.is_err() {
        let err = unsafe { GetLastError() };
        return Err(io::Error::from_raw_os_error(err.0 as i32));
    }
    Ok(())
}

/// Tap a key: down → wait `hold` → up. The down/up pair is sequential
/// PostMessage calls; if the down succeeds but up fails the next call
/// to `find_cs2_window` will still succeed (CS2's WindowProc handles
/// it idempotently — we are not toying with the OS-level keyboard
/// state).
pub fn post_key_tap(target: Cs2Window, vk: u8, hold: Duration) -> io::Result<()> {
    post_key_event(target, vk, false)?;
    sleep(at_least(hold));
    post_key_event(target, vk, true)
}

/// Send one Unicode character into CS2's chat input.
///
/// Each UTF-16 unit becomes its own `WM_CHAR`, which SDL2 forwards as
/// an `SDL_TEXTINPUT` event — the same path the in-game IME uses, so
/// CJK input "just works" without touching the clipboard.
pub fn post_char(target: Cs2Window, c: char) -> io::Result<()> {
    let mut buf = [0u16; 2];
    for unit in c.encode_utf16(&mut buf).iter().copied() {
        let r = unsafe { PostMessageW(target.hwnd(), WM_CHAR, WPARAM(unit as usize), LPARAM(1)) };
        if r.is_err() {
            let err = unsafe { GetLastError() };
            return Err(io::Error::from_raw_os_error(err.0 as i32));
        }
    }
    Ok(())
}

/// Tap an ASCII character against an already-resolved CS2 window.
pub fn post_char_key(target: Cs2Window, ch: char, hold: Duration) -> io::Result<()> {
    post_key_tap(target, char_to_vk(ch), hold)
}

/// Tap Enter against an already-resolved CS2 window.
pub fn post_enter(target: Cs2Window) -> io::Result<()> {
    post_key_tap(target, VK_RETURN, MIN_KEY_INTERVAL)
}

/// Convenience: tap a single ASCII char.
pub fn press_key(ch: char, delay: Duration) -> io::Result<()> {
    let target = find_cs2_window().ok_or_else(|| io::Error::other("CS2 main window not found"))?;
    post_key_tap(target, char_to_vk(ch), delay)
}

/// Tap a key by spec (`"ins"`, `"y"`, `"f5"`, …). Returns `Ok(false)`
/// if the spec is unknown.
pub fn press_key_spec(spec: &str, delay: Duration) -> io::Result<bool> {
    let Some(vk) = spec_to_vk(spec) else {
        return Ok(false);
    };
    let target = find_cs2_window().ok_or_else(|| io::Error::other("CS2 main window not found"))?;
    post_key_tap(target, vk, delay)?;
    Ok(true)
}

/// Tap Enter once.
pub fn press_enter() -> io::Result<()> {
    press_key('\r', MIN_KEY_INTERVAL)
}

/// Type `text` into CS2's chat box, character by character. The chat
/// box must already be open (the caller has tapped the chat key first).
/// `per_char` controls the spacing between WM_CHAR posts.
pub fn type_text(text: &str, per_char: Duration) -> io::Result<()> {
    let target = find_cs2_window().ok_or_else(|| io::Error::other("CS2 main window not found"))?;
    for c in text.chars() {
        post_char(target, c)?;
        sleep(at_least(per_char));
    }
    Ok(())
}

/// Milliseconds since the user's last keyboard or mouse input. Used
/// only as a fairness check before we start typing — even though
/// PostMessage doesn't *collide* with real input on the OS level, two
/// sources writing into the same chat box at once still produces
/// gibberish.
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
/// process. With PostMessage we don't actually require this for the
/// dispatch to succeed, but it's still a useful hint for the user
/// ("did you alt-tab away?").
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

// Suppress unused-imports warning from PWSTR if the Windows API
// surface changes.
#[allow(dead_code)]
fn _force_link(_: PWSTR) {}
