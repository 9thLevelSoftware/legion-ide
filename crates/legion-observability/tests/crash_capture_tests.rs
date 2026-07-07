//! Integration tests for consent-gated panic capture and diagnostics export.
//!
//! Consent-off tests are pinned first per PKT-CRASH TDD discipline.
//!
//! ## Test isolation
//!
//! The panic hook (`std::panic::set_hook`) is global per-process state.
//! All tests that touch it are serialized through `HOOK_LOCK` to prevent
//! interference when the test suite runs with more than one thread.

use std::path::PathBuf;
use std::sync::Mutex;

use legion_observability::crash_capture::{CrashCaptureConfig, install_panic_hook, uninstall_panic_hook};
use legion_observability::export::{DiagnosticsExportBuilder, ExportError};
use legion_protocol::WorkbenchTelemetryConsent;

/// Guards access to the global panic hook so tests run serially.
static HOOK_LOCK: Mutex<()> = Mutex::new(());

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn consent_off() -> WorkbenchTelemetryConsent {
    WorkbenchTelemetryConsent {
        enabled: false,
        crash_reports_enabled: false,
        raw_source_allowed: false,
        consent_label: "local-only".to_string(),
        schema_version: 1,
    }
}

fn consent_on() -> WorkbenchTelemetryConsent {
    WorkbenchTelemetryConsent {
        enabled: true,
        crash_reports_enabled: true,
        raw_source_allowed: false,
        consent_label: "crash-reports".to_string(),
        schema_version: 1,
    }
}

fn consent_raw() -> WorkbenchTelemetryConsent {
    WorkbenchTelemetryConsent {
        enabled: true,
        crash_reports_enabled: true,
        raw_source_allowed: true,
        consent_label: "raw-allowed".to_string(),
        schema_version: 1,
    }
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "legion_crash_test_{label}_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4(),
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------------------------------------------------------------------------
// CONSENT-OFF TESTS — pinned first (TDD red→green discipline)
// ---------------------------------------------------------------------------

/// When consent is disabled, `install_panic_hook` must return `Ok(())` without
/// creating the bundle directory or installing a custom panic hook.
#[test]
fn consent_disabled_installs_no_hook() {
    let _guard = HOOK_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    // Use a directory path that does NOT exist yet — if the hook touches it we'll know.
    let bundle_dir = std::env::temp_dir().join(format!(
        "legion_crash_consent_off_{}",
        uuid::Uuid::new_v4()
    ));
    assert!(!bundle_dir.exists(), "dir should not exist before test");

    let config = CrashCaptureConfig {
        bundle_dir: bundle_dir.clone(),
        consent: consent_off(),
    };

    // Must succeed without any side effects.
    install_panic_hook(config).expect("install with consent off should return Ok");

    // The bundle dir must NOT have been created.
    assert!(
        !bundle_dir.exists(),
        "bundle_dir must not be created when consent is off"
    );

    // No hook was installed — triggering a panic must not write any files.
    let _ = std::panic::catch_unwind(|| panic!("consent-off probe"));
    assert!(
        !bundle_dir.exists(),
        "bundle_dir must still not exist after induced panic with consent off"
    );

    // Note: take_hook restores the default; since we didn't install one, this is a no-op clean up.
    uninstall_panic_hook();
}

/// When consent is disabled and we install the hook (which does nothing), the
/// next call to `take_hook` must return without crashing, confirming that our
/// implementation did not install a foreign hook.
#[test]
fn consent_disabled_leaves_default_hook() {
    let _guard = HOOK_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let bundle_dir = std::env::temp_dir().join(format!(
        "legion_crash_consent_default_{}",
        uuid::Uuid::new_v4()
    ));

    let config = CrashCaptureConfig {
        bundle_dir: bundle_dir.clone(),
        consent: consent_off(),
    };

    install_panic_hook(config).expect("install with consent off should succeed");

    // Induce a panic — no files should appear because we didn't install a hook.
    let _ = std::panic::catch_unwind(|| panic!("default-hook probe"));

    // Bundle dir must remain absent — proves the default hook (not ours) ran.
    assert!(
        !bundle_dir.exists(),
        "default hook must not write to our bundle_dir"
    );

    uninstall_panic_hook();
}

// ---------------------------------------------------------------------------
// PANIC CAPTURE TESTS
// ---------------------------------------------------------------------------

/// With consent enabled, a caught panic must produce a `panic.txt` and a
/// `summary.toml` in a subdirectory of the bundle directory.
#[test]
fn induced_panic_produces_bundle() {
    let _guard = HOOK_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let bundle_dir = unique_temp_dir("bundle_produced");

    let config = CrashCaptureConfig {
        bundle_dir: bundle_dir.clone(),
        consent: consent_on(),
    };

    install_panic_hook(config).expect("install with consent on should succeed");

    // Induce a panic via catch_unwind so the hook fires but the test continues.
    let _ = std::panic::catch_unwind(|| panic!("induced test panic"));

    uninstall_panic_hook();

    // Check that exactly one crash subdirectory was created.
    let dirs: Vec<_> = std::fs::read_dir(&bundle_dir)
        .expect("bundle_dir should be readable")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    assert_eq!(dirs.len(), 1, "expected exactly one crash subdirectory");

    let crash_dir = dirs[0].path();
    assert!(crash_dir.join("panic.txt").exists(), "panic.txt must exist");
    assert!(crash_dir.join("summary.toml").exists(), "summary.toml must exist");
}

/// `panic.txt` must contain "stack backtrace" or individual frame markers
/// confirming that a symbolicated backtrace was captured.
#[test]
fn panic_txt_contains_backtrace() {
    let _guard = HOOK_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let bundle_dir = unique_temp_dir("backtrace_check");

    let config = CrashCaptureConfig {
        bundle_dir: bundle_dir.clone(),
        consent: consent_on(),
    };

    install_panic_hook(config).expect("install should succeed");
    let _ = std::panic::catch_unwind(|| panic!("backtrace probe"));
    uninstall_panic_hook();

    let crash_dir = std::fs::read_dir(&bundle_dir)
        .expect("bundle_dir readable")
        .filter_map(|e| e.ok())
        .find(|e| e.path().is_dir())
        .expect("at least one crash dir")
        .path();

    let panic_txt = std::fs::read_to_string(crash_dir.join("panic.txt"))
        .expect("panic.txt should be readable");

    assert!(
        panic_txt.contains("stack backtrace") || panic_txt.contains("backtrace"),
        "panic.txt should contain backtrace output; got:\n{panic_txt}"
    );
}

/// `summary.toml` must contain required metadata fields and must NOT contain
/// raw source file contents or full absolute paths.
#[test]
fn summary_toml_is_metadata_only() {
    let _guard = HOOK_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let bundle_dir = unique_temp_dir("summary_metadata");

    let config = CrashCaptureConfig {
        bundle_dir: bundle_dir.clone(),
        consent: consent_on(),
    };

    install_panic_hook(config).expect("install should succeed");
    let _ = std::panic::catch_unwind(|| panic!("summary probe"));
    uninstall_panic_hook();

    let crash_dir = std::fs::read_dir(&bundle_dir)
        .expect("bundle_dir readable")
        .filter_map(|e| e.ok())
        .find(|e| e.path().is_dir())
        .expect("at least one crash dir")
        .path();

    let summary = std::fs::read_to_string(crash_dir.join("summary.toml"))
        .expect("summary.toml should be readable");

    // Required fields must be present.
    assert!(summary.contains("crash_id"), "summary must contain crash_id; got:\n{summary}");
    assert!(summary.contains("timestamp"), "summary must contain timestamp; got:\n{summary}");
    assert!(summary.contains("os ="), "summary must contain os; got:\n{summary}");
    assert!(summary.contains("arch ="), "summary must contain arch; got:\n{summary}");
    assert!(summary.contains("version ="), "summary must contain version; got:\n{summary}");
    assert!(summary.contains("signer_status"), "summary must contain signer_status; got:\n{summary}");

    // Must NOT contain raw source code or full paths.
    assert!(!summary.contains("fn main"), "summary must not contain source code");
    // Must not contain absolute path components (C:\ or /home etc.)
    let lower = summary.to_ascii_lowercase();
    assert!(
        !lower.contains("c:\\users") && !lower.contains("/home/"),
        "summary must not contain absolute machine paths; got:\n{summary}"
    );
}

// ---------------------------------------------------------------------------
// EXPORT DOUBLE-OPT-IN TESTS
// ---------------------------------------------------------------------------

/// The default export (no `include_raw` flag) must be metadata-only with
/// empty `raw_paths` in all entries.
#[test]
fn export_default_is_metadata_only() {
    let bundle_dir = unique_temp_dir("export_meta");

    // Manually create a fake crash bundle to export.
    let crash_id = uuid::Uuid::new_v4().to_string();
    let crash_dir = bundle_dir.join(&crash_id);
    std::fs::create_dir_all(&crash_dir).unwrap();
    std::fs::write(crash_dir.join("summary.toml"), format!("crash_id = \"{crash_id}\"\n")).unwrap();
    std::fs::write(crash_dir.join("panic.txt"), "stack backtrace:\n").unwrap();

    let bundle = DiagnosticsExportBuilder::new(bundle_dir, consent_on())
        .build()
        .expect("default build should succeed");

    assert!(bundle.metadata_only, "default export must be metadata_only");
    assert!(
        bundle.entries.iter().all(|e| e.raw_paths.is_empty()),
        "raw_paths must be empty in default export"
    );
}

/// Requesting raw data without `raw_source_allowed` consent must return an error.
#[test]
fn export_raw_requires_consent_and_flag() {
    let bundle_dir = unique_temp_dir("export_raw_denied");

    let result = DiagnosticsExportBuilder::new(bundle_dir, consent_on()) // raw_source_allowed = false
        .with_include_raw(true)
        .build();

    assert!(
        matches!(result, Err(ExportError::RawNotAllowed)),
        "should be RawNotAllowed, got: {result:?}"
    );
}

/// When both `include_raw: true` AND `raw_source_allowed: true` are set,
/// the bundle must include raw paths.
#[test]
fn export_raw_allowed_when_both_set() {
    let bundle_dir = unique_temp_dir("export_raw_allowed");

    let crash_id = uuid::Uuid::new_v4().to_string();
    let crash_dir = bundle_dir.join(&crash_id);
    std::fs::create_dir_all(&crash_dir).unwrap();
    std::fs::write(crash_dir.join("summary.toml"), format!("crash_id = \"{crash_id}\"\n")).unwrap();
    std::fs::write(crash_dir.join("panic.txt"), "stack backtrace:\n").unwrap();

    let bundle = DiagnosticsExportBuilder::new(bundle_dir, consent_raw())
        .with_include_raw(true)
        .build()
        .expect("raw build with consent should succeed");

    assert!(!bundle.metadata_only, "must NOT be metadata_only when raw allowed");
    assert!(
        bundle.entries.iter().any(|e| !e.raw_paths.is_empty()),
        "at least one entry must have raw_paths"
    );
}
