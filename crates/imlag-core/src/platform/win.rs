//! Windows implementations of keyboard automation and foreground-window
//! detection. Direct Win32 calls via the `windows` crate — no `user32!{}`
//! manual extern blocks.

#![allow(unsafe_code)]

use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, HMODULE, HWND};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, GetKeyboardState, MapVirtualKeyW, VkKeyScanW, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    MAPVK_VK_TO_VSC, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

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

fn key_event(vk: u8, up: bool) {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u8;
    let flags = if up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    unsafe {
        keybd_event(vk, scan, flags, 0);
    }
}

/// Send a single press-and-release. The `delay` argument controls both the
/// hold time and the rest before returning.
pub fn press_key(ch: char, delay: Duration) {
    let vk = char_to_vk(ch);
    key_event(vk, false);
    sleep(delay);
    key_event(vk, true);
    sleep(delay);
}

/// Hold a key down without releasing.
pub fn key_down(ch: char) {
    key_event(char_to_vk(ch), false);
}

/// Release a previously held key.
pub fn key_up(ch: char) {
    key_event(char_to_vk(ch), true);
}

/// Release WASD/Space/Shift/Ctrl/Alt and the two main mouse buttons. Used
/// before injecting a chat sequence, so the player's own held keys don't
/// interfere with the typing.
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
        0x01, // LMB
        0x02, // RMB
    ];
    for k in KEYS {
        key_event(*k, true);
    }
    sleep(Duration::from_millis(50));
}

/// Release every key the OS currently considers pressed.
///
/// Walks the 256-entry keyboard state from `GetKeyboardState`, sends a
/// `KEYUP` for each VK whose high bit is set. Avoids spamming `keyup` for
/// keys the user wasn't holding, so IME / modifier toggle state stays clean.
///
/// Skips VK 0 (unused) and the 0xE? OEM cluster's well-known bogus codes
/// that some keyboards report as stuck.
pub fn release_all_keys() {
    let mut state = [0u8; 256];
    if unsafe { GetKeyboardState(&mut state) }.is_err() {
        // Fallback to the smaller well-known set if the syscall fails.
        release_movement_keys();
        return;
    }
    let mut released = 0u32;
    for vk in 1u16..=254u16 {
        // Skip VK_PACKET (0xE7) — it isn't a physical key, it's used for
        // injected unicode strokes and "releasing" it can produce stray
        // characters in the focused control.
        if vk == 0xE7 {
            continue;
        }
        if state[vk as usize] & 0x80 != 0 {
            key_event(vk as u8, true);
            released += 1;
        }
    }
    if released > 0 {
        sleep(Duration::from_millis(30));
    }
}

/// Type the standard "select all + delete" sequence into the focused control.
pub fn clear_input(delay: Duration) {
    key_down('\u{11}');
    sleep(Duration::from_millis(50));
    press_key('A', Duration::from_millis(50));
    key_up('\u{11}');
    sleep(delay);
    press_key('\u{2E}', delay); // Delete
}

/// Type the standard Ctrl+V paste shortcut.
pub fn paste_clipboard() {
    key_down('\u{11}');
    std::thread::sleep(Duration::from_millis(50));
    press_key('V', Duration::from_millis(50));
    key_up('\u{11}');
}

/// Press the Enter key once.
pub fn press_enter() {
    press_key('\u{0D}', Duration::from_millis(50));
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
