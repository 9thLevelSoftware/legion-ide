use std::{fs, path::PathBuf};

use legion_desktop::package::{
    LEGAL_NOTICE_FILES, PACKAGE_MANIFEST_NAME, PackageProfile, WINDOWS_EXECUTABLE_NAME,
    WindowsPackageConfig, copy_legal_notices, package_manifest, plan_windows_package,
    write_package_manifest,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn package_plan_points_at_expected_debug_executable() {
    let root = workspace_root();
    let output = root.join("target/gui-phase6-package");
    let config = WindowsPackageConfig::new(root.clone(), output.clone(), PackageProfile::Debug);

    let plan = plan_windows_package(&config).expect("package plan should resolve");

    assert_eq!(plan.cargo_args, ["build", "-p", "legion-desktop"]);
    assert_eq!(
        plan.executable_source,
        root.join("target")
            .join("debug")
            .join(WINDOWS_EXECUTABLE_NAME)
    );
    assert_eq!(plan.package_dir, output);
    assert_eq!(
        plan.executable_destination,
        plan.package_dir.join(WINDOWS_EXECUTABLE_NAME)
    );
    assert_eq!(
        plan.manifest_path,
        plan.package_dir.join(PACKAGE_MANIFEST_NAME)
    );
}

#[test]
fn package_plan_points_at_expected_release_executable() {
    let root = workspace_root();
    let config = WindowsPackageConfig::new(
        root.clone(),
        root.join("target/gui-phase6-package-release"),
        PackageProfile::Release,
    );

    let plan = plan_windows_package(&config).expect("package plan should resolve");

    assert_eq!(
        plan.cargo_args,
        ["build", "-p", "legion-desktop", "--release"]
    );
    assert_eq!(
        plan.executable_source,
        root.join("target")
            .join("release")
            .join(WINDOWS_EXECUTABLE_NAME)
    );
}

#[test]
fn package_manifest_is_metadata_only_and_redacts_source_payloads() {
    let root = workspace_root();
    let config = WindowsPackageConfig::new(
        root.clone(),
        root.join("target/gui-phase6-package"),
        PackageProfile::Debug,
    );
    let plan = plan_windows_package(&config).expect("package plan should resolve");

    let manifest = package_manifest(&plan, true);

    assert!(manifest.contains("package: legion-desktop"));
    assert!(manifest.contains("platform: windows"));
    assert!(manifest.contains("dry_run: true"));
    assert!(manifest.contains("cargo_command: cargo build -p legion-desktop"));
    assert!(manifest.contains(WINDOWS_EXECUTABLE_NAME));
    assert!(manifest.contains("legal_license: LICENSE"));
    assert!(manifest.contains("legal_privacy: PRIVACY.md"));
    assert!(manifest.contains("legal_third_party_notices: THIRD_PARTY_NOTICES.md"));
    assert!(!manifest.contains("small_buffer_preview"));
    assert!(!manifest.contains("source_body"));
}

#[test]
fn package_manifest_write_creates_only_manifest_metadata() {
    let root = workspace_root();
    let output = std::env::temp_dir().join(format!(
        "legion_desktop_package_test_{}",
        std::process::id()
    ));
    let config = WindowsPackageConfig::new(root, output.clone(), PackageProfile::Debug);
    let plan = plan_windows_package(&config).expect("package plan should resolve");

    write_package_manifest(&plan, true).expect("manifest should be written");

    let manifest = fs::read_to_string(output.join(PACKAGE_MANIFEST_NAME))
        .expect("manifest should be readable");
    assert!(manifest.contains("dry_run: true"));
    assert!(!output.join(WINDOWS_EXECUTABLE_NAME).exists());
    for (_, dest) in LEGAL_NOTICE_FILES {
        let path = output.join(dest);
        assert!(path.is_file(), "missing packaged legal file {dest}");
        let body = fs::read_to_string(&path).expect("legal file should be readable");
        assert!(!body.is_empty(), "{dest} should not be empty");
    }

    if output.starts_with(std::env::temp_dir())
        && output
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("legion_desktop_package_test_"))
    {
        let _ = fs::remove_dir_all(output);
    }
}

#[test]
fn copy_legal_notices_ships_license_privacy_and_third_party_notices() {
    let root = workspace_root();
    let output = std::env::temp_dir().join(format!(
        "legion_desktop_legal_notices_{}",
        std::process::id()
    ));
    copy_legal_notices(&root, &output).expect("legal notices should copy");

    let license = fs::read_to_string(output.join("LICENSE")).expect("LICENSE");
    assert!(
        license.contains("proprietary"),
        "LICENSE must be proprietary"
    );
    assert!(
        !license
            .to_ascii_lowercase()
            .contains("permission is hereby granted"),
        "LICENSE must not imply an OSI/MIT grant"
    );

    let privacy = fs::read_to_string(output.join("PRIVACY.md")).expect("PRIVACY.md");
    assert!(privacy.contains("zero egress") || privacy.contains("zero-egress"));
    assert!(privacy.contains("opt-in"));
    assert!(privacy.contains("no phone-home"));

    let notices =
        fs::read_to_string(output.join("THIRD_PARTY_NOTICES.md")).expect("THIRD_PARTY_NOTICES.md");
    assert!(notices.contains("SmallCode"));

    if output.starts_with(std::env::temp_dir())
        && output
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("legion_desktop_legal_notices_"))
    {
        let _ = fs::remove_dir_all(output);
    }
}
