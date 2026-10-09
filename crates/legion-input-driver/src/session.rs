//! The interactive-desktop probe.
//!
//! ADR-0056's rule: availability is **probed, never inferred**. `cfg!(windows)`
//! is not evidence that a desktop session exists, and neither is an environment
//! variable that happens to be set on a developer's machine. A headless Windows
//! CI runner, or a process running in a service session, must report that it
//! cannot attach. `UOI_IO` checks the current attachment; otherwise
//! `OpenInputDesktop` / `SetThreadDesktop` must establish one.

/// The outcome of attempting to attach this process to the input desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopAttachment {
    /// The driver really attached its thread to the input desktop.
    Attached {
        /// Name of the desktop that was attached, as the OS reports it.
        desktop: String,
    },
    /// A supported host that has no interactive input desktop to attach to.
    NotAttached {
        /// What the OS said, verbatim enough to act on.
        detail: String,
    },
    /// A host this driver cannot inject OS-level input on at all.
    UnsupportedHost {
        /// Which host, and why it is out of scope.
        detail: String,
    },
}

/// Attach to the input desktop, or report why that was impossible.
///
/// On Windows this first checks whether the thread's existing desktop receives
/// input. If so, retain that attachment and its granted access. Otherwise open
/// the input desktop and attach; inability to query or attach remains blocked.
#[cfg(windows)]
pub fn probe_input_desktop() -> DesktopAttachment {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS, DESKTOP_CREATEWINDOW,
        DESKTOP_READOBJECTS, DESKTOP_WRITEOBJECTS, GetThreadDesktop, GetUserObjectInformationW,
        OpenInputDesktop, SetThreadDesktop, UOI_IO, UOI_NAME,
    };
    use windows::Win32::System::Threading::GetCurrentThreadId;

    // STA COM bootstrap creates a hidden window after SetThreadDesktop. The
    // attached handle must permit it; this requests one extra object right,
    // without changing the desktop ACL or bypassing attachment failure.
    let access = DESKTOP_ACCESS_FLAGS(
        DESKTOP_READOBJECTS.0 | DESKTOP_WRITEOBJECTS.0 | DESKTOP_CREATEWINDOW.0,
    );
    // SAFETY: plain Win32 calls with owned arguments; the returned handle is
    // checked before use and this process exits shortly after the probe.
    unsafe {
        let current = match GetThreadDesktop(GetCurrentThreadId()) {
            Ok(desktop) => desktop,
            Err(err) => {
                return DesktopAttachment::NotAttached {
                    detail: format!("GetThreadDesktop failed: {err}"),
                };
            }
        };
        let mut receives_input = 0u32;
        if let Err(err) = GetUserObjectInformationW(
            HANDLE(current.0),
            UOI_IO,
            Some((&mut receives_input as *mut u32).cast()),
            std::mem::size_of_val(&receives_input) as u32,
            None,
        ) {
            return DesktopAttachment::NotAttached {
                detail: format!("query current desktop input status failed: {err}"),
            };
        }
        let desktop = if receives_input != 0 {
            // Reopening the same input desktop with a reduced mask and calling
            // SetThreadDesktop would replace this thread's existing access.
            // GetThreadDesktop returns a borrowed handle: do not close it.
            current
        } else {
            let desktop = match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, access) {
                Ok(desktop) => desktop,
                Err(err) => {
                    return DesktopAttachment::NotAttached {
                        detail: format!("OpenInputDesktop failed: {err}"),
                    };
                }
            };
            if let Err(err) = SetThreadDesktop(desktop) {
                let _ = CloseDesktop(desktop);
                return DesktopAttachment::NotAttached {
                    detail: format!("SetThreadDesktop failed: {err}"),
                };
            }
            desktop
        };

        // Retain the borrowed handle or keep the newly opened attachment alive
        // until process exit. No ACL, token, or desktop activation is changed.
        let mut name = [0u16; 128];
        let mut needed = 0u32;
        let byte_length = u32::try_from(std::mem::size_of_val(&name)).unwrap_or(0);
        let desktop_name = match GetUserObjectInformationW(
            HANDLE(desktop.0),
            UOI_NAME,
            Some(name.as_mut_ptr().cast()),
            byte_length,
            Some(&mut needed),
        ) {
            Ok(()) => {
                let end = name
                    .iter()
                    .position(|unit| *unit == 0)
                    .unwrap_or(name.len());
                String::from_utf16_lossy(&name[..end])
            }
            Err(_) => String::new(),
        };

        DesktopAttachment::Attached {
            desktop: desktop_name,
        }
    }
}

/// Non-Windows hosts have no injection path enabled by ADR-0056.
///
/// This is never a pass. `main` turns it into a blocked report naming
/// `BLK-2026-09-08-02` and a nonzero exit.
#[cfg(not(windows))]
pub fn probe_input_desktop() -> DesktopAttachment {
    DesktopAttachment::UnsupportedHost {
        detail: format!(
            "`{}` is not a native input host enabled by ADR-0056; only Windows is",
            std::env::consts::OS
        ),
    }
}
