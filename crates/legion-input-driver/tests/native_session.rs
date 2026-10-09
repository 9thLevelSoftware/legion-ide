//! Opt-in OS attachment regression. No product, UIA client or input is launched.
#![cfg(windows)]

#[path = "../src/session.rs"]
#[allow(dead_code)]
mod session;

use windows::Win32::{
    Foundation::HANDLE,
    System::{
        StationsAndDesktops::{GetThreadDesktop, GetUserObjectInformationW, UOI_IO},
        Threading::GetCurrentThreadId,
    },
};

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryObject(
        handle: *mut core::ffi::c_void,
        information_class: u32,
        information: *mut core::ffi::c_void,
        length: u32,
        needed: *mut u32,
    ) -> i32;
}

fn granted_access(handle: HANDLE) -> u32 {
    // PUBLIC_OBJECT_BASIC_INFORMATION: Attributes, GrantedAccess, counts,
    // then reserved fields. Only the public GrantedAccess value is observed.
    let mut information = [0u32; 14];
    let mut needed = 0;
    let status = unsafe {
        NtQueryObject(
            handle.0,
            0,
            information.as_mut_ptr().cast(),
            std::mem::size_of_val(&information) as u32,
            &mut needed,
        )
    };
    assert!(status >= 0, "query desktop access: NTSTATUS {status:#x}");
    information[1]
}

#[test]
#[ignore = "requires an interactive Windows input desktop; run isolated with --ignored"]
fn already_attached_input_desktop_preserves_existing_access() {
    let before = unsafe { GetThreadDesktop(GetCurrentThreadId()) }.unwrap();
    let mut receives_input = 0u32;
    unsafe {
        GetUserObjectInformationW(
            HANDLE(before.0),
            UOI_IO,
            Some((&mut receives_input as *mut u32).cast()),
            std::mem::size_of_val(&receives_input) as u32,
            None,
        )
    }
    .unwrap();
    assert_ne!(receives_input, 0, "test requires the current input desktop");
    let before_access = granted_access(HANDLE(before.0));
    assert_ne!(
        before_access & !0x83,
        0,
        "test requires inherited access beyond the restricted reopen mask"
    );

    assert!(matches!(
        session::probe_input_desktop(),
        session::DesktopAttachment::Attached { .. }
    ));

    let after = unsafe { GetThreadDesktop(GetCurrentThreadId()) }.unwrap();
    assert_eq!(
        granted_access(HANDLE(after.0)),
        before_access,
        "probing an already attached input desktop must not reduce its granted access"
    );
}
