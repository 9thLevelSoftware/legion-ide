//! Real-process sandbox escape tests.
//!
//! These tests spawn actual child processes via `spawn_sandboxed` and assert
//! on observable effects (files present/absent, exit codes, enforcement
//! reports) — not just advisory API return values.
//!
//! Platform-specific tests are gated with `#[cfg(target_os = "...")]`.
//! Cross-platform tests (e.g. SBPL generation) run everywhere.

// ---------------------------------------------------------------------------
// Windows enforcement tests
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod windows_tests {
    use legion_sandbox::spawn::{SandboxSpawnSpec, spawn_sandboxed};
    use std::path::PathBuf;
    use std::time::Duration;

    /// Return the path to the pre-built sandbox-escape-probe binary.
    ///
    /// Cargo sets `CARGO_BIN_EXE_<name>` (hyphens → underscores) at test
    /// runtime in the test environment, so the binary is already compiled.
    fn probe_binary() -> PathBuf {
        // Cargo sets this env var when running tests in the same package.
        if let Ok(path) = std::env::var("CARGO_BIN_EXE_sandbox_escape_probe") {
            let p = PathBuf::from(path);
            if p.exists() {
                return p;
            }
        }
        // Fallback: locate via CARGO_MANIFEST_DIR → workspace root → target/debug/
        let mut path = PathBuf::from(
            std::env::var("CARGO_MANIFEST_DIR")
                .expect("CARGO_MANIFEST_DIR must be set during tests"),
        );
        path.pop(); // crates/
        path.pop(); // repo root
        path.push("target");
        path.push("debug");
        path.push("sandbox-escape-probe.exe");
        if !path.exists() {
            // Build it if needed
            let output = std::process::Command::new("cargo")
                .args([
                    "build",
                    "--bin",
                    "sandbox-escape-probe",
                    "-p",
                    "legion-sandbox",
                ])
                .output()
                .expect("cargo build");
            assert!(
                output.status.success(),
                "probe build failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(
            path.exists(),
            "probe binary not found at {}",
            path.display()
        );
        path
    }

    /// Positive control: the probe can write inside the writable root.
    #[test]
    fn positive_control_write_inside_writable_root_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let target_file = dir.path().join("allowed.txt");
        let probe = probe_binary();

        let spec = SandboxSpawnSpec {
            program: probe,
            args: vec![
                "write".to_string(),
                target_file.to_string_lossy().to_string(),
            ],
            working_dir: dir.path().to_path_buf(),
            writable_root: dir.path().to_path_buf(),
            allowed_egress: Default::default(),
            timeout: Duration::from_secs(30),
            env: vec![],
        };

        let result = spawn_sandboxed(&spec).expect("spawn succeeds");
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(
            stdout.contains("WRITE_OK"),
            "positive control: write inside root should succeed, got: {stdout}"
        );
        assert!(
            target_file.exists(),
            "file should exist after write inside root"
        );
        assert!(!result.timed_out, "should not time out");
    }

    /// Windows job-object-only implementation is honest: filesystem writes are
    /// NOT enforced. The enforcement report documents this limitation, and the
    /// residual is real (C2 cut line): a write outside `writable_root` succeeds.
    #[test]
    fn write_outside_writable_root_enforcement_is_honest() {
        let writable = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target_file = outside.path().join("escaped.txt");
        let probe = probe_binary();

        let spec = SandboxSpawnSpec {
            program: probe,
            args: vec![
                "write".to_string(),
                target_file.to_string_lossy().to_string(),
            ],
            working_dir: writable.path().to_path_buf(),
            writable_root: writable.path().to_path_buf(),
            allowed_egress: Default::default(),
            timeout: Duration::from_secs(30),
            env: vec![],
        };

        let result = spawn_sandboxed(&spec).expect("spawn succeeds");
        let stdout = String::from_utf8_lossy(&result.stdout);
        // Residual risk (C2): job object does not block outside-root writes.
        assert!(
            stdout.contains("WRITE_OK") && target_file.exists(),
            "C2 residual: outside write must succeed under job-object-only, got stdout={stdout}, exists={}",
            target_file.exists()
        );
        // The enforcement report must be honest — not claim enforcement that isn't there.
        assert!(
            !result.enforcement.filesystem_write_enforced,
            "windows job-object-only: filesystem_write_enforced must be false (honest)"
        );
        // Caveat labels must document the limitation.
        assert!(
            result
                .enforcement
                .caveat_labels
                .iter()
                .any(|c| c.contains("filesystem") || c.contains("token")),
            "caveat labels must document the lack of filesystem enforcement: {:?}",
            result.enforcement.caveat_labels
        );
    }

    /// Network is NOT enforced on Windows (job-object-only path).
    /// The enforcement report must say so honestly.
    #[test]
    fn network_caveat_is_honest_on_windows() {
        let dir = tempfile::tempdir().unwrap();
        let probe = probe_binary();

        let spec = SandboxSpawnSpec {
            program: probe,
            // Port 1 is effectively unreachable but the connection attempt is fast
            args: vec!["connect".to_string(), "127.0.0.1:1".to_string()],
            working_dir: dir.path().to_path_buf(),
            writable_root: dir.path().to_path_buf(),
            allowed_egress: Default::default(),
            timeout: Duration::from_secs(10),
            env: vec![],
        };

        let result = spawn_sandboxed(&spec).expect("spawn succeeds");
        // Windows does NOT enforce network — the report must say so.
        assert!(
            !result.enforcement.network_enforced,
            "windows: network_enforced must be false"
        );
        assert!(
            result
                .enforcement
                .caveat_labels
                .iter()
                .any(|c| c.contains("network")),
            "caveat labels must document missing network enforcement: {:?}",
            result.enforcement.caveat_labels
        );
    }

    /// The enforcement report must name the backend used.
    #[test]
    fn enforcement_report_documents_backend() {
        let dir = tempfile::tempdir().unwrap();
        let probe = probe_binary();

        let spec = SandboxSpawnSpec {
            program: probe,
            args: vec![
                "write".to_string(),
                dir.path().join("test.txt").to_string_lossy().to_string(),
            ],
            working_dir: dir.path().to_path_buf(),
            writable_root: dir.path().to_path_buf(),
            allowed_egress: Default::default(),
            timeout: Duration::from_secs(10),
            env: vec![],
        };

        let result = spawn_sandboxed(&spec).expect("spawn succeeds");
        assert!(
            !result.enforcement.backend_used.is_empty(),
            "backend_used must be non-empty"
        );
        assert!(
            result.enforcement.backend_used.contains("job-object"),
            "backend_used should identify the job-object backend, got: {}",
            result.enforcement.backend_used
        );
    }

    /// Job object with KILL_ON_JOB_CLOSE must kill the child when the timeout
    /// fires — even if the child is a long-running process.
    #[test]
    fn job_object_kills_child_on_timeout() {
        let dir = tempfile::tempdir().unwrap();

        let spec = SandboxSpawnSpec {
            program: PathBuf::from("cmd.exe"),
            // ping loops for 60 iterations (≈ 60 s) — well beyond the 2 s timeout
            args: vec!["/C".to_string(), "ping -n 60 127.0.0.1".to_string()],
            working_dir: dir.path().to_path_buf(),
            writable_root: dir.path().to_path_buf(),
            allowed_egress: Default::default(),
            timeout: Duration::from_secs(2),
            env: vec![],
        };

        let result = spawn_sandboxed(&spec).expect("spawn succeeds");
        assert!(
            result.timed_out,
            "process should have been killed on timeout"
        );
        assert_eq!(result.exit_code, None, "timed-out process has no exit code");
    }
}

// ---------------------------------------------------------------------------
// Cross-platform SBPL generation tests
// ---------------------------------------------------------------------------

#[test]
fn sbpl_profile_denies_default_and_allows_writable_root() {
    use legion_sandbox::spawn::generate_sbpl_profile;
    let profile = generate_sbpl_profile(
        std::path::Path::new("/workspace/project"),
        &Default::default(),
    );
    assert!(
        profile.contains("(deny default)"),
        "profile must deny by default"
    );
    assert!(
        profile.contains("/workspace/project"),
        "profile must reference writable root"
    );
    assert!(
        profile.contains("file-write*"),
        "profile must allow file-write*"
    );
}

#[test]
fn sbpl_profile_with_egress_allows_specified_hosts() {
    use legion_sandbox::spawn::generate_sbpl_profile;
    let mut egress = std::collections::BTreeSet::new();
    egress.insert("api.anthropic.com:443".to_string());
    let profile = generate_sbpl_profile(std::path::Path::new("/workspace"), &egress);
    assert!(
        profile.contains("api.anthropic.com"),
        "profile must contain the allowed egress host"
    );
}

#[test]
fn sbpl_profile_with_no_egress_denies_network() {
    use legion_sandbox::spawn::generate_sbpl_profile;
    let profile = generate_sbpl_profile(std::path::Path::new("/workspace"), &Default::default());
    assert!(
        profile.contains("(deny network*)"),
        "profile with no egress must deny all network"
    );
}

/// A writable_root containing SBPL metacharacters must not inject arbitrary
/// SBPL rules into the generated profile.
#[test]
fn sbpl_profile_escapes_injection_in_writable_root() {
    use legion_sandbox::spawn::generate_sbpl_profile;
    // Path containing a `"` that would prematurely close the SBPL string and
    // an injected allow-network rule.
    let profile = generate_sbpl_profile(
        std::path::Path::new("/tmp/work\") (allow network*) (#"),
        &Default::default(),
    );
    // Security property: no top-level SBPL line may equal `(allow network*)`.
    // The embedded `"` must be escaped to `\"`, keeping the entire path inside
    // the quoted string rather than breaking out as a free-standing directive.
    assert!(
        !profile
            .lines()
            .any(|line| line.trim() == "(allow network*)"),
        "SBPL injection via writable_root must not create a top-level rule; profile:\n{profile}"
    );
    // The `"` in the path must be visible as `\"` in the emitted profile.
    assert!(
        profile.contains("\\\""),
        "embedded double-quote must be escaped to `\\\"` in SBPL output; profile:\n{profile}"
    );
    // The unconditional deny-network rule must still be present.
    assert!(
        profile.contains("(deny network*)"),
        "deny network* must still be present after escaping; profile:\n{profile}"
    );
}

/// An egress entry containing SBPL metacharacters must be escaped.
#[test]
fn sbpl_profile_escapes_injection_in_egress_entry() {
    use legion_sandbox::spawn::generate_sbpl_profile;
    let mut egress = std::collections::BTreeSet::new();
    // Egress entry that tries to break out of the tcp string and add a free rule
    egress.insert("evil.example.com\") (allow file-write* (subpath \"/\"))".to_string());
    let profile = generate_sbpl_profile(std::path::Path::new("/workspace"), &egress);
    // The injected file-write* rule must NOT appear as a bare SBPL directive
    assert!(
        !profile.contains("(allow file-write* (subpath \"/\"))"),
        "SBPL injection via egress entry must be escaped; profile:\n{profile}"
    );
    // The egress entry itself must still appear (escaped) in the profile
    assert!(
        profile.contains("evil.example.com"),
        "escaped egress hostname must still appear in profile"
    );
}
