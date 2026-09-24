//! The interactive-desktop probe.
//!
//! ADR-0056's rule: availability is **probed, never inferred**. `cfg!(windows)`
//! is not evidence that a desktop session exists, and neither is an environment
//! variable that happens to be set on a developer's machine. A headless Windows
//! CI runner, or a process running in a service session, must report that it
//! cannot attach — which is exactly what `OpenInputDesktop` /
//! `SetThreadDesktop` do when there is no input desktop to attach to.

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
/// On Windows this opens the *input* desktop (the one currently receiving
/// keyboard and mouse input) and makes it this thread's desktop. Both calls
/// fail in a service session and on a station with no visible desktop, which is
/// the property the blocked path depends on.
#[cfg(windows)]
pub fn probe_input_desktop() -> DesktopAttachment {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS,
        DESKTOP_WRITEOBJECTS, GetUserObjectInformationW, OpenInputDesktop, SetThreadDesktop,
        UOI_NAME,
    };

    let access = DESKTOP_ACCESS_FLAGS(DESKTOP_READOBJECTS.0 | DESKTOP_WRITEOBJECTS.0);
    // SAFETY: plain Win32 calls with owned arguments; the returned handle is
    // checked before use and this process exits shortly after the probe.
    unsafe {
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

        // The handle deliberately stays open: it is now this thread's desktop,
        // and closing it would detach the thread that is about to inject.
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
