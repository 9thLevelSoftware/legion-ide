//! Out-of-process observation: the UI Automation oracle and the on-disk bytes
//! oracle.
//!
//! Every function here reads the product from *outside* its process. Nothing in
//! this module links the product, calls into it, or asks it a question through
//! a channel the product knows about. ADR-0056's out-of-process rule is not a
//! style preference: an assertion inside the product cannot tell a working
//! native input path from a harness talking to itself.
//!
//! The SHA-256 implementation is deliberately in-crate rather than a
//! dependency. This crate takes exactly one external crate — `windows` — so its
//! dependency-policy admission stays as narrow as an acceptance instrument can
//! be. `tests/driver_contract.rs` pins it against the published FIPS 180-4
//! known-answer vectors.
//!
//! The Windows half sits at file scope behind per-item `#[cfg(windows)]` rather
//! than in an inner module re-exported with `pub use`. `tests/driver_contract.rs`
//! includes this file by path and uses only the host-independent digest half, so
//! a `pub use windows_uia::*;` there is an unused import in that target and fails
//! `clippy -D warnings`. Keep the items at file scope; do not reintroduce the
//! re-export.

use std::path::Path;

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let mut message = bytes.to_vec();
    let bit_length = (bytes.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());

    for block in message.as_chunks::<64>().0 {
        let mut schedule = [0u32; 64];
        for (index, word) in schedule.iter_mut().take(16).enumerate() {
            let start = index * 4;
            *word = u32::from_be_bytes([
                block[start],
                block[start + 1],
                block[start + 2],
                block[start + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for (constant, word) in K.iter().zip(schedule.iter()) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choose)
                .wrapping_add(*constant)
                .wrapping_add(*word);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }

    let mut hex = String::with_capacity(64);
    for word in state {
        for byte in word.to_be_bytes() {
            hex.push_str(&format!("{byte:02x}"));
        }
    }
    hex
}

/// Code points at or above U+3000, used as the IME/CJK presence signal.
pub fn cjk_chars(text: &str) -> impl Iterator<Item = char> + '_ {
    text.chars().filter(|ch| *ch as u32 >= 0x3000)
}

/// True when `observed` contains more CJK-range characters than `baseline`.
///
/// The IME class runs after text and clipboard checks that already insert
/// characters at or above U+3000. Comparing against a snapshot taken immediately
/// before the IME sequence is what stops those earlier markers from counting as
/// a successful composition.
pub fn has_new_cjk(baseline: &str, observed: &str) -> bool {
    cjk_chars(observed).count() > cjk_chars(baseline).count()
}

/// Map a virtual-screen pixel coordinate into the 0..=65535 range `SendInput`
/// uses with `MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK`.
pub fn absolute_pointer_from_virtual_screen(
    x: i32,
    y: i32,
    origin_x: i32,
    origin_y: i32,
    width: i32,
    height: i32,
) -> Result<(i32, i32), String> {
    if width <= 1 || height <= 1 {
        return Err(format!(
            "the virtual screen reports {width}x{height}; there is no coordinate space to \
             click in"
        ));
    }
    let absolute_x = (i64::from(x - origin_x) * 65535) / i64::from(width - 1);
    let absolute_y = (i64::from(y - origin_y) * 65535) / i64::from(height - 1);
    Ok((
        i32::try_from(absolute_x.clamp(0, 65535)).unwrap_or(0),
        i32::try_from(absolute_y.clamp(0, 65535)).unwrap_or(0),
    ))
}

/// The on-disk oracle: the file's byte length and SHA-256, read by the driver
/// from a host shell, never through the product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDigest {
    /// Lowercase hex SHA-256 of the file's bytes as they sit on disk.
    pub sha256: String,
    /// The bytes themselves, so a marker can be searched for.
    pub bytes: Vec<u8>,
}

/// Read `path` and digest it. An unreadable file is an error, never an
/// assumed-empty digest.
pub fn file_digest(path: &Path) -> Result<FileDigest, String> {
    let bytes = std::fs::read(path)
        .map_err(|err| format!("cannot read {} from disk: {err}", path.display()))?;
    Ok(FileDigest {
        sha256: sha256_hex(&bytes),
        bytes,
    })
}

#[cfg(windows)]
use std::time::{Duration, Instant};

#[cfg(windows)]
use windows::core::BOOL;
#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
#[cfg(windows)]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
#[cfg(windows)]
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
    TreeScope_Subtree, UIA_TextPatternId,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
};

/// Cap on how much UI Automation text one read returns.
#[cfg(windows)]
const MAX_TEXT: i32 = 1 << 16;

#[cfg(windows)]
struct WindowSearch {
    process_id: u32,
    found: HWND,
}

#[cfg(windows)]
unsafe extern "system" fn collect_window(window: HWND, param: LPARAM) -> BOOL {
    // SAFETY: `param` is the `&mut WindowSearch` this callback was started
    // with, and `EnumWindows` runs the callback synchronously on this
    // thread, so the borrow is live for the whole enumeration.
    let search = unsafe { &mut *(param.0 as *mut WindowSearch) };
    let mut owner = 0u32;
    // SAFETY: `window` comes from the enumeration itself.
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut owner));
        if owner == search.process_id && IsWindowVisible(window).as_bool() {
            search.found = window;
            return BOOL(0);
        }
    }
    BOOL(1)
}

/// Wait for a **visible top-level window owned by `process_id`**, observed
/// from outside that process by enumerating the desktop's windows.
///
/// `process_exited` is polled between enumerations. When it returns `Some`, the
/// product process has already terminated and the wait stops immediately
/// instead of burning the remaining timeout and reporting a missing-host
/// window. `None` from this function still means "no window appeared while the
/// process stayed alive".
#[cfg(windows)]
pub enum WindowWait {
    /// A visible top-level window owned by the product process.
    Found(HWND),
    /// The product process exited before opening a window.
    ProcessExited {
        /// Process exit code, or 1 when the OS did not report one.
        exit_code: i32,
    },
    /// The timeout elapsed while the process was still running.
    TimedOut,
}

/// Wait for a visible top-level window owned by `process_id`.
#[cfg(windows)]
pub fn wait_for_window(
    process_id: u32,
    timeout: Duration,
    mut process_exited: impl FnMut() -> Option<i32>,
) -> WindowWait {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(exit_code) = process_exited() {
            return WindowWait::ProcessExited { exit_code };
        }
        let mut search = WindowSearch {
            process_id,
            found: HWND(std::ptr::null_mut()),
        };
        let param = LPARAM(&raw mut search as isize);
        // SAFETY: the callback is a plain `extern "system"` function and
        // `param` points at a live local for the duration of the call.
        let _ = unsafe { EnumWindows(Some(collect_window), param) };
        if !search.found.is_invalid() {
            return WindowWait::Found(search.found);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if let Some(exit_code) = process_exited() {
        WindowWait::ProcessExited { exit_code }
    } else {
        WindowWait::TimedOut
    }
}

/// The UI Automation client, held for the life of a conformance run.
#[cfg(windows)]
pub struct UiaOracle {
    automation: IUIAutomation,
}

#[cfg(windows)]
impl UiaOracle {
    /// Initialise COM for this thread and create the UI Automation client.
    pub fn open() -> Result<Self, String> {
        // SAFETY: both calls are the documented COM bootstrap sequence.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|err| format!("CoInitializeEx failed: {err}"))?;
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                    .map_err(|err| format!("CoCreateInstance(CUIAutomation) failed: {err}"))?;
            Ok(Self { automation })
        }
    }

    /// The automation element for a window handle.
    pub fn element_from_window(&self, window: HWND) -> Result<IUIAutomationElement, String> {
        // SAFETY: `window` was produced by the desktop enumeration above.
        unsafe {
            self.automation
                .ElementFromHandle(window)
                .map_err(|err| format!("ElementFromHandle failed: {err}"))
        }
    }

    /// Every element in `root`'s subtree, in tree order.
    fn subtree(&self, root: &IUIAutomationElement) -> Result<Vec<IUIAutomationElement>, String> {
        // SAFETY: plain UI Automation reads against a live element.
        unsafe {
            let condition = self
                .automation
                .CreateTrueCondition()
                .map_err(|err| format!("CreateTrueCondition failed: {err}"))?;
            let found = root
                .FindAll(TreeScope_Subtree, &condition)
                .map_err(|err| format!("FindAll failed: {err}"))?;
            let length = found
                .Length()
                .map_err(|err| format!("element array length failed: {err}"))?;
            let mut elements = Vec::new();
            for index in 0..length {
                if let Ok(element) = found.GetElement(index) {
                    elements.push(element);
                }
            }
            Ok(elements)
        }
    }

    /// The first element in `root`'s subtree that exposes `TextPattern`.
    ///
    /// If there is none, the UI Automation text oracle does not exist on
    /// this build and the caller must report the affected classes blocked.
    pub fn text_element(
        &self,
        root: &IUIAutomationElement,
    ) -> Result<(IUIAutomationElement, IUIAutomationTextPattern), String> {
        for element in self.subtree(root)? {
            // SAFETY: `element` is live and the pattern id is a constant.
            let pattern = unsafe {
                element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
            };
            if let Ok(pattern) = pattern {
                return Ok((element, pattern));
            }
        }
        Err(
            "no element under the product window exposes a UI Automation TextPattern, so \
             the out-of-process text oracle cannot be established"
                .to_string(),
        )
    }

    /// The whole document text of a `TextPattern`, read from outside the
    /// product process.
    pub fn document_text(&self, pattern: &IUIAutomationTextPattern) -> Result<String, String> {
        // SAFETY: `pattern` is a live UI Automation pattern.
        unsafe {
            let range = pattern
                .DocumentRange()
                .map_err(|err| format!("TextPattern::DocumentRange failed: {err}"))?;
            let text = range
                .GetText(MAX_TEXT)
                .map_err(|err| format!("TextRange::GetText failed: {err}"))?;
            Ok(text.to_string())
        }
    }

    /// The current selection text, or the empty string when the pattern
    /// reports no selected range.
    pub fn selection_text(&self, pattern: &IUIAutomationTextPattern) -> Result<String, String> {
        // SAFETY: `pattern` is a live UI Automation pattern.
        unsafe {
            let ranges = pattern
                .GetSelection()
                .map_err(|err| format!("TextPattern::GetSelection failed: {err}"))?;
            let length = ranges
                .Length()
                .map_err(|err| format!("selection length failed: {err}"))?;
            let mut text = String::new();
            for index in 0..length {
                if let Ok(range) = ranges.GetElement(index)
                    && let Ok(value) = range.GetText(MAX_TEXT)
                {
                    text.push_str(&value.to_string());
                }
            }
            Ok(text)
        }
    }

    /// The element's bounding rectangle in screen coordinates. This is the
    /// only geometry the pointer class is allowed to click at.
    pub fn bounding_rectangle(&self, element: &IUIAutomationElement) -> Result<RECT, String> {
        // SAFETY: `element` is live.
        unsafe {
            element
                .CurrentBoundingRectangle()
                .map_err(|err| format!("CurrentBoundingRectangle failed: {err}"))
        }
    }

    /// The element under the keyboard focus, read from outside the product.
    pub fn focused_element(&self) -> Result<IUIAutomationElement, String> {
        // SAFETY: plain UI Automation read.
        unsafe {
            self.automation
                .GetFocusedElement()
                .map_err(|err| format!("GetFocusedElement failed: {err}"))
        }
    }

    /// A stable digest of every `Name` in `root`'s subtree.
    ///
    /// This is the driver's dirty-affordance oracle: it does not know which
    /// element the product uses to show unsaved state, only whether the
    /// accessible names the product publishes changed. A snapshot that
    /// never changes across an edit means the affordance is not visible
    /// from outside the process, which is reported blocked, not conforming.
    pub fn name_snapshot(&self, root: &IUIAutomationElement) -> Result<String, String> {
        let mut names = String::new();
        for element in self.subtree(root)? {
            // SAFETY: `element` is live.
            if let Ok(name) = unsafe { element.CurrentName() } {
                names.push_str(&name.to_string());
                names.push('\u{1f}');
            }
        }
        Ok(sha256_hex(names.as_bytes()))
    }
}
