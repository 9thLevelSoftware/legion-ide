//! OS-level input injection: `SendInput` and the Win32 clipboard API.
//!
//! Nothing in this module knows anything about the product. Every event goes
//! through the same OS path a human's keyboard and mouse use, and the clipboard
//! it writes is the *system* clipboard, not any product abstraction. That is
//! the whole point of ADR-0056: an injector that talked to the product directly
//! could not tell a working native input path from a harness talking to itself.

use std::{thread, time::Duration};

use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MOUSEEVENTF_ABSOLUTE,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK,
    MOUSEINPUT, MOUSE_EVENT_FLAGS, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

/// `CF_UNICODETEXT`. Spelled as the raw clipboard format number the Win32 API
/// takes so this crate does not have to enable the `Win32_System_Ole` feature
/// for one integer.
pub const CF_UNICODETEXT_FORMAT: u32 = 13;

/// Pause between injected events, so the product's event loop actually sees
/// them as separate input rather than one coalesced burst.
const EVENT_GAP: Duration = Duration::from_millis(30);

/// Push a batch of already-built events through `SendInput`.
fn send(inputs: &[INPUT]) -> Result<(), String> {
    if inputs.is_empty() {
        return Ok(());
    }
    let size = i32::try_from(size_of::<INPUT>())
        .map_err(|_| "INPUT is larger than SendInput can describe".to_string())?;
    // SAFETY: `inputs` is a live slice of fully initialised `INPUT` values and
    // `size` is the size of that type.
    let sent = unsafe { SendInput(inputs, size) };
    if sent as usize != inputs.len() {
        return Err(format!(
            "SendInput accepted {sent} of {} events; the input desktop rejected the rest",
            inputs.len()
        ));
    }
    thread::sleep(EVENT_GAP);
    Ok(())
}

fn key_event(key: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn unicode_event(unit: u16, extra_flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: KEYBD_EVENT_FLAGS(KEYEVENTF_UNICODE.0 | extra_flags),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Press and release one virtual key.
pub fn key_press(key: VIRTUAL_KEY) -> Result<(), String> {
    send(&[
        key_event(key, KEYBD_EVENT_FLAGS(0)),
        key_event(key, KEYEVENTF_KEYUP),
    ])
}

/// Press and release one virtual key that lives in the extended key set (the
/// navigation cluster, for example `Home` and `End`).
pub fn extended_key_press(key: VIRTUAL_KEY) -> Result<(), String> {
    send(&[
        key_event(key, KEYEVENTF_EXTENDEDKEY),
        key_event(
            key,
            KEYBD_EVENT_FLAGS(KEYEVENTF_EXTENDEDKEY.0 | KEYEVENTF_KEYUP.0),
        ),
    ])
}

/// Hold `modifiers` down, press and release `key`, then release the modifiers
/// in reverse order. This is the OS chord route, not a synthesized command.
pub fn chord(modifiers: &[VIRTUAL_KEY], key: VIRTUAL_KEY) -> Result<(), String> {
    let mut events = Vec::with_capacity(modifiers.len() * 2 + 2);
    for modifier in modifiers {
        events.push(key_event(*modifier, KEYBD_EVENT_FLAGS(0)));
    }
    events.push(key_event(key, KEYBD_EVENT_FLAGS(0)));
    events.push(key_event(key, KEYEVENTF_KEYUP));
    for modifier in modifiers.iter().rev() {
        events.push(key_event(*modifier, KEYEVENTF_KEYUP));
    }
    send(&events)
}

/// Type `text` as Unicode key events, one UTF-16 code unit at a time.
///
/// Surrogate pairs are sent as two units, so a multi-code-point grapheme
/// reaches the product exactly as the OS would deliver it from a keyboard.
pub fn unicode_text(text: &str) -> Result<(), String> {
    for unit in text.encode_utf16() {
        send(&[
            unicode_event(unit, 0),
            unicode_event(unit, KEYEVENTF_KEYUP.0),
        ])?;
    }
    Ok(())
}

/// Click the primary pointer button at an absolute screen coordinate.
///
/// The coordinate must have been derived from the UI Automation bounding
/// rectangle of the target element. ADR-0056 forbids taking it from any
/// product geometry API.
pub fn click_at(x: i32, y: i32) -> Result<(), String> {
    // SAFETY: `GetSystemMetrics` takes a plain index and returns a plain int.
    let (origin_x, origin_y, width, height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    let (dx, dy) = crate::observe::absolute_pointer_from_virtual_screen(
        x, y, origin_x, origin_y, width, height,
    )?;
    let absolute_flags = MOUSEEVENTF_MOVE.0 | MOUSEEVENTF_ABSOLUTE.0 | MOUSEEVENTF_VIRTUALDESK.0;

    let mouse = |flags: u32| INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: MOUSE_EVENT_FLAGS(flags),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    send(&[mouse(absolute_flags)])?;
    send(&[mouse(
        MOUSEEVENTF_LEFTDOWN.0 | MOUSEEVENTF_ABSOLUTE.0 | MOUSEEVENTF_VIRTUALDESK.0,
    )])?;
    send(&[mouse(
        MOUSEEVENTF_LEFTUP.0 | MOUSEEVENTF_ABSOLUTE.0 | MOUSEEVENTF_VIRTUALDESK.0,
    )])
}

/// Put `text` on the **system** clipboard as `CF_UNICODETEXT`.
pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    let mut utf16: Vec<u16> = text.encode_utf16().collect();
    utf16.push(0);
    let bytes = utf16.len() * size_of::<u16>();

    // SAFETY: every handle is checked before use and the clipboard is closed on
    // every path out of this block.
    unsafe {
        OpenClipboard(None).map_err(|err| format!("OpenClipboard failed: {err}"))?;

        if let Err(err) = EmptyClipboard() {
            let _ = CloseClipboard();
            return Err(format!("EmptyClipboard failed: {err}"));
        }

        let global = match GlobalAlloc(GMEM_MOVEABLE, bytes) {
            Ok(global) => global,
            Err(err) => {
                let _ = CloseClipboard();
                return Err(format!("GlobalAlloc failed: {err}"));
            }
        };

        let destination = GlobalLock(global);
        if destination.is_null() {
            let _ = CloseClipboard();
            return Err("GlobalLock returned no pointer for the clipboard buffer".to_string());
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr(), destination.cast::<u16>(), utf16.len());
        let _ = GlobalUnlock(global);

        // On success the clipboard owns the block; on failure this leaks one
        // small buffer for the remaining life of this short-lived process,
        // which is preferable to freeing memory the OS may already own.
        let result = SetClipboardData(CF_UNICODETEXT_FORMAT, Some(HANDLE(global.0)))
            .map(|_| ())
            .map_err(|err| format!("SetClipboardData failed: {err}"));
        let _ = CloseClipboard();
        result
    }
}

/// Read the **system** clipboard back as text. This is the oracle for a copy
/// gesture; the product's own clipboard abstraction is never consulted.
pub fn read_clipboard_text() -> Result<String, String> {
    /// Refuse to walk past this many UTF-16 units looking for a terminator.
    const MAX_UNITS: usize = 1 << 20;

    // SAFETY: the handle is checked before it is locked and the clipboard is
    // closed on every path out of this block.
    unsafe {
        OpenClipboard(None).map_err(|err| format!("OpenClipboard failed: {err}"))?;

        let handle = match GetClipboardData(CF_UNICODETEXT_FORMAT) {
            Ok(handle) => handle,
            Err(err) => {
                let _ = CloseClipboard();
                return Err(format!("GetClipboardData(CF_UNICODETEXT) failed: {err}"));
            }
        };

        let global = HGLOBAL(handle.0);
        let source = GlobalLock(global).cast::<u16>();
        if source.is_null() {
            let _ = CloseClipboard();
            return Err("GlobalLock returned no pointer for the clipboard buffer".to_string());
        }

        let size = GlobalSize(global);
        if size < size_of::<u16>() {
            let _ = GlobalUnlock(global);
            let _ = CloseClipboard();
            return Err(
                "clipboard CF_UNICODETEXT block is smaller than one UTF-16 unit".to_string(),
            );
        }
        let max_units = (size / size_of::<u16>()).min(MAX_UNITS);

        let mut length = 0usize;
        while length < max_units && *source.add(length) != 0 {
            length += 1;
        }
        if length == max_units {
            let _ = GlobalUnlock(global);
            let _ = CloseClipboard();
            return Err(
                "clipboard CF_UNICODETEXT has no terminator inside the allocated block".to_string(),
            );
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(source, length));

        let _ = GlobalUnlock(global);
        let _ = CloseClipboard();
        Ok(text)
    }
}
