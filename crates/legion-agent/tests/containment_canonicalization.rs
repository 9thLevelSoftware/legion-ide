//! Containment canonicalization: `validate_containment` must resolve
//! symlinks consistently on BOTH sides of the check.
//!
//! Pre-fix defects (uncovered during the 3-OS CI bring-up):
//! - False REJECT: a symlink-aliased base (macOS `/var` -> `/private/var`
//!   style) canonicalizes on the base side only, so every in-sandbox path
//!   spelled through the alias fails `strip_prefix`.
//! - False ALLOW: an existing symlink INSIDE the sandbox pointing outside
//!   passes the purely lexical path check while real file operations follow
//!   it out of the sandbox.

use std::fs;
use std::path::PathBuf;

use legion_agent::validate_containment;

fn unique_temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "legion-containment-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[cfg(unix)]
fn make_symlink_dir(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn make_symlink_dir(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    // Requires Developer Mode or admin; callers skip gracefully when denied.
    std::os::windows::fs::symlink_dir(target, link)
}

/// macOS `/var` style: the base is reached through a symlink alias. Both the
/// base and the target are the SAME real location, so containment must accept.
#[test]
fn containment_accepts_symlink_aliased_base() {
    let real_root = unique_temp_dir("aliased-real");
    fs::create_dir_all(real_root.join("src")).expect("create src");
    fs::write(real_root.join("src/lib.rs"), "pub fn x() {}\n").expect("write file");

    let alias_parent = unique_temp_dir("aliased-link-parent");
    let alias = alias_parent.join("alias");
    if make_symlink_dir(&real_root, &alias).is_err() {
        eprintln!("skipping: symlink creation not permitted on this host");
        let _ = fs::remove_dir_all(&real_root);
        let _ = fs::remove_dir_all(&alias_parent);
        return;
    }

    let relative = validate_containment(&alias, &alias.join("src/lib.rs"))
        .expect("alias-spelled path inside the aliased base must be contained");
    assert_eq!(relative, PathBuf::from("src").join("lib.rs"));

    let _ = fs::remove_dir_all(&alias_parent);
    let _ = fs::remove_dir_all(&real_root);
}

/// An existing symlink inside the sandbox that points OUTSIDE must be
/// rejected: following it would escape the sandbox even though the lexical
/// path looks contained.
#[test]
fn containment_rejects_existing_symlink_escaping_sandbox() {
    let sandbox = unique_temp_dir("escape-sandbox");
    let outside = unique_temp_dir("escape-outside");
    fs::write(outside.join("secret.txt"), "outside\n").expect("write outside file");

    let link = sandbox.join("link");
    if make_symlink_dir(&outside, &link).is_err() {
        eprintln!("skipping: symlink creation not permitted on this host");
        let _ = fs::remove_dir_all(&sandbox);
        let _ = fs::remove_dir_all(&outside);
        return;
    }

    let result = validate_containment(&sandbox, &sandbox.join("link/secret.txt"));
    assert!(
        result.is_err(),
        "path through an in-sandbox symlink to an outside dir must be rejected, got {result:?}"
    );

    let _ = fs::remove_dir_all(&sandbox);
    let _ = fs::remove_dir_all(&outside);
}

/// Proposals may target files that do not exist yet: a non-existent path
/// under the base must be accepted with the correct relative remainder.
#[test]
fn containment_accepts_nonexistent_target_inside_base() {
    let sandbox = unique_temp_dir("newfile-sandbox");
    let relative = validate_containment(&sandbox, &sandbox.join("src/new_module.rs"))
        .expect("non-existent in-sandbox target must be contained");
    assert_eq!(relative, PathBuf::from("src").join("new_module.rs"));
    let _ = fs::remove_dir_all(&sandbox);
}

/// Lexical traversal on a non-existent path must still be rejected.
#[test]
fn containment_rejects_nonexistent_traversal() {
    let sandbox = unique_temp_dir("traversal-sandbox");
    assert!(validate_containment(&sandbox, &sandbox.join("../escape.txt")).is_err());
    let _ = fs::remove_dir_all(&sandbox);
}

/// A dangling symlink as the resolved component must fail closed: writing
/// "through" it would create the target at an unverified location.
#[test]
fn containment_rejects_dangling_symlink_component() {
    let sandbox = unique_temp_dir("dangling-sandbox");
    let gone = unique_temp_dir("dangling-target");
    let link = sandbox.join("dangling");
    if make_symlink_dir(&gone, &link).is_err() {
        eprintln!("skipping: symlink creation not permitted on this host");
        let _ = fs::remove_dir_all(&sandbox);
        let _ = fs::remove_dir_all(&gone);
        return;
    }
    fs::remove_dir_all(&gone).expect("remove target to dangle the link");

    let result = validate_containment(&sandbox, &sandbox.join("dangling/file.txt"));
    assert!(
        result.is_err(),
        "dangling symlink component must fail closed, got {result:?}"
    );

    let _ = fs::remove_dir_all(&sandbox);
}
