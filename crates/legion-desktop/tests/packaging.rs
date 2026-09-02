use std::{fs, path::PathBuf};

use legion_desktop::package::{
    LEGAL_NOTICE_FILES, PACKAGE_MANIFEST_NAME, PackageProfile, PackageSku,
    UNSIGNED_BETA_SIGNER_STATUS, WINDOWS_EXECUTABLE_NAME, WindowsPackageConfig, copy_legal_notices,
    package_manifest, plan_windows_package, write_package_manifest,
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

    assert_eq!(plan.sku, PackageSku::Default);
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
    assert!(manifest.contains("sku: default"));
    assert!(manifest.contains(&format!("signer_status: {UNSIGNED_BETA_SIGNER_STATUS}")));
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

#[test]
fn manual_sku_build_is_offline_and_not_the_default_ai_desktop() {
    let root = workspace_root();
    let config = WindowsPackageConfig::new(
        root.clone(),
        root.join("target/gui-phase6-package-manual"),
        PackageProfile::Release,
    )
    .with_sku(PackageSku::Manual);
    let plan = plan_windows_package(&config).expect("manual package plan");

    assert_eq!(plan.sku, PackageSku::Manual);
    assert_eq!(
        plan.cargo_args,
        [
            "build",
            "-p",
            "legion-desktop",
            "--no-default-features",
            "--features",
            "offline",
            "--release"
        ]
    );
    let manifest = package_manifest(&plan, true);
    assert!(manifest.contains("sku: manual"));
    assert!(!manifest.contains("sku: default"));
    assert!(manifest.contains("--no-default-features"));
    assert!(manifest.contains("--features offline"));
}

#[test]
fn default_sku_cargo_args_do_not_enable_offline() {
    let root = workspace_root();
    let config = WindowsPackageConfig::new(
        root.clone(),
        root.join("target/gui-phase6-package"),
        PackageProfile::Debug,
    );
    let plan = plan_windows_package(&config).expect("default package plan");
    assert_eq!(plan.sku, PackageSku::Default);
    assert!(!plan.cargo_args.iter().any(|arg| arg == "offline"));
    assert!(
        !plan
            .cargo_args
            .iter()
            .any(|arg| arg == "--no-default-features")
    );
}

#[test]
fn desktop_manual_sku_does_not_declare_provider_http_as_a_required_dep() {
    let cargo_toml = fs::read_to_string(workspace_root().join("crates/legion-desktop/Cargo.toml"))
        .expect("desktop Cargo.toml");
    assert!(
        cargo_toml.contains("legion-ai-providers = { workspace = true, optional = true }"),
        "Manual SKU must not require legion-ai-providers as a product feature"
    );
    assert!(
        cargo_toml.contains("\"dep:legion-ai-providers\""),
        "provider HTTP must stay behind the ai feature"
    );
    assert!(
        cargo_toml.contains("offline = [\"legion-app/offline\"]"),
        "offline SKU must not enable legion-ai-providers"
    );
}

#[test]
fn native_package_scripts_accept_manual_sku() {
    let root = workspace_root();
    let sh = fs::read_to_string(root.join("scripts/package-native.sh")).expect("package-native.sh");
    assert!(sh.contains("--sku"));
    assert!(sh.contains("--no-default-features"));
    assert!(sh.contains("sku = \"$SKU\""));
    let ps1 =
        fs::read_to_string(root.join("scripts/package-native.ps1")).expect("package-native.ps1");
    assert!(ps1.contains("[string]$Sku"));
    assert!(ps1.contains("--no-default-features"));
    assert!(ps1.contains("sku ="));
}
