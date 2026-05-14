use devil_security::{PathAccess, PathPolicy};

#[test]
fn sibling_prefix_escape_is_rejected_cross_platform() {
    let policy = PathPolicy {
        writable_roots: vec!["/repo/root".to_string()],
        readable_roots: vec!["/repo/root".to_string()],
        blocked_roots: vec![],
        max_write_bytes: 1_024,
    };

    assert!(policy.can_access("/repo/root/src/main.rs", PathAccess::Read));
    assert!(!policy.can_access("/repo/root-evil/src/main.rs", PathAccess::Read));
}

#[cfg(windows)]
#[test]
fn windows_drive_letter_case_and_slash_normalization_are_supported() {
    let policy = PathPolicy {
        writable_roots: vec!["C:/Repo/Root".to_string()],
        readable_roots: vec!["C:/Repo/Root".to_string()],
        blocked_roots: vec![],
        max_write_bytes: 1_024,
    };

    assert!(policy.can_access("c:\\repo\\root\\src\\main.rs", PathAccess::Write));
    assert!(policy.can_access("C:/REPO/ROOT/src/lib.rs", PathAccess::Read));
}

#[cfg(windows)]
#[test]
fn windows_long_path_prefix_is_supported() {
    let policy = PathPolicy {
        writable_roots: vec!["C:/repo/root".to_string()],
        readable_roots: vec!["C:/repo/root".to_string()],
        blocked_roots: vec![],
        max_write_bytes: 1_024,
    };

    assert!(policy.can_access("\\\\?\\C:\\repo\\root\\src\\main.rs", PathAccess::Read));
}

#[cfg(windows)]
#[test]
fn windows_blocked_root_precedence_is_enforced() {
    let policy = PathPolicy {
        writable_roots: vec!["C:/repo".to_string()],
        readable_roots: vec!["C:/repo".to_string()],
        blocked_roots: vec!["c:/repo/secret".to_string()],
        max_write_bytes: 1_024,
    };

    assert!(!policy.can_access("C:\\repo\\secret\\notes.txt", PathAccess::Write));
    assert!(policy.can_access("C:\\repo\\public\\notes.txt", PathAccess::Write));
}
