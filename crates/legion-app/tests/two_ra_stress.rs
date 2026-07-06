//! rust-analyzer wedge reproduction harness (PKT-S3-WEDGE-R3).
//!
//! Drives rapid didChange→publish cycles on session A (initialized like the
//! GP-1 probe, with `files.watcher: client`) optionally alongside session B
//! (initialized like the product lazy-start session, plain `initialize`) on
//! the same temp workspace, failing with full round-3 post-mortem evidence
//! (reader stats, exit status, buffered notifications, stderr rings for
//! both sessions) if a cycle times out or a process dies.
//!
//! This harness found both s3 wedge root causes (see the evidence file at
//! plans/evidence/production/M8/PKT-S3-WEDGE-R3-evidence.md):
//! 1. URI drive-designator mismatch — rust-analyzer echoes lowercase-drive
//!    URIs; raw-string fingerprint matching never matched them (Windows).
//!    Reproduced solo — the two-RA topology was NOT required.
//! 2. Cache-priming starvation — RA's prime-caches std indexing holds salsa
//!    queries that demand-driven diagnostics block on; publishes stall past
//!    the pump deadline with an empty stderr ring. With priming disabled
//!    (`STRESS_NO_PRIME=1`, and now the GP-1 probe default) ten error→clear
//!    cycles complete in ~3 s that otherwise wedge at cycle 2.
//!
//! `#[ignore]`d: this spawns real rust-analyzer processes and loops for
//! minutes. Run explicitly:
//!
//! ```text
//! cargo test -p legion-app --test two_ra_stress -- --ignored --nocapture
//! ```
//!
//! Knobs:
//! - `STRESS_CYCLES` (default 15): error→clear cycles to drive.
//! - `STRESS_SOLO=1`: skip session B (single-RA control run).
//! - `STRESS_DEADLINE_SECS` (default 30): per-phase diagnostics deadline.
//! - `STRESS_NO_PRIME=1`: disable rust-analyzer cache priming on session A.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use legion_app::language::{
    DiscoveredBinary, RustAnalyzerDiscovery, RustAnalyzerLaunchConfig, RustAnalyzerSession,
};
use legion_lsp::{LspServerProcessConfig, LspStdioLauncher, LspSupervisorConfig};
use legion_protocol::{
    CapabilityDecisionId, CapabilityId, CausalityId, CorrelationId, FileFingerprint, LanguageId,
    LanguageServerId, LspConfiguredServerIdentity, LspLaunchPolicyDecision,
    LspWorkspaceTrustPosture, RedactionHint, SemanticPrivacyScope, WorkspaceId, WorkspaceRootId,
    WorkspaceTrustState,
};
use uuid::Uuid;

const ERROR_TEXT_TEMPLATE: &str =
    "// stress cycle {N}\npub fn scratchpad() {\n    let _x: i32 = \"stress_type_error\";\n}\n";
const FIXED_TEXT_TEMPLATE: &str = "// stress cycle {N} fixed\npub fn scratchpad() {}\n";

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("create {dst:?}: {e}"))?;
    for entry in fs::read_dir(src).map_err(|e| format!("read {src:?}: {e}"))? {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        let ty = entry.file_type().map_err(|e| format!("file type: {e}"))?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to).map_err(|e| format!("copy to {to:?}: {e}"))?;
        }
    }
    Ok(())
}

fn path_to_file_uri(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if normalized.starts_with('/') {
        format!("file://{normalized}")
    } else {
        format!("file:///{normalized}")
    }
}

fn launch_policy(command: &str) -> LspLaunchPolicyDecision {
    let identity = LspConfiguredServerIdentity {
        server_id: LanguageServerId(902),
        workspace_id: WorkspaceId(1),
        root_id: Some(WorkspaceRootId(1)),
        language_id: LanguageId("rust".to_string()),
        display_name: "rust-analyzer-stress".to_string(),
        command_hash: FileFingerprint {
            algorithm: "stress".to_string(),
            value: format!("cmd:{command}"),
        },
        args_hash: None,
        env_hash: None,
        cwd_hash: None,
        settings_hash: None,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let posture = LspWorkspaceTrustPosture {
        workspace_id: WorkspaceId(1),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        privacy_scope: SemanticPrivacyScope::Workspace,
        privacy_scope_allowed: true,
        required_capability: CapabilityId("process.spawn".to_string()),
        decision_id: Some(CapabilityDecisionId(1)),
        diagnostics: Vec::new(),
        schema_version: 1,
    };
    LspLaunchPolicyDecision::evaluate(
        identity,
        posture,
        true,
        CorrelationId(1),
        CausalityId(Uuid::from_u128(902)),
        Vec::new(),
        1,
    )
}

fn spawn_session(workspace: &Path, binary: &Path, watcher_client: bool) -> RustAnalyzerSession {
    let command = binary.to_string_lossy().into_owned();
    let config = RustAnalyzerLaunchConfig {
        discovery: RustAnalyzerDiscovery {
            configured_path: Some(binary.to_path_buf()),
            ..Default::default()
        },
        supervisor: LspSupervisorConfig {
            launch_policy: launch_policy(&command),
            process: LspServerProcessConfig {
                command,
                args: Vec::new(),
                cwd: Some(workspace.to_path_buf()),
                // STRESS_RA_LOG=1 makes rust-analyzer narrate to stderr; the
                // ring (last 100 lines) then shows exactly where a stalled
                // run stopped (PKT-S3-WEDGE-R3 forensics). Opt-in because the
                // tracing overhead massively distorts timing.
                env: if std::env::var("STRESS_RA_LOG").as_deref() == Ok("1") {
                    vec![("RA_LOG".to_string(), "info".to_string())]
                } else {
                    Vec::new()
                },
            },
            initial_backoff_ms: 50,
            max_backoff_ms: 1000,
            max_restart_attempts: 1,
        },
        server_id: LanguageServerId(902),
        language_id: LanguageId("rust".to_string()),
    };
    let mut launcher = LspStdioLauncher::new();
    let mut session = RustAnalyzerSession::launch(config, &mut launcher).expect("launch RA");
    let root_uri = path_to_file_uri(workspace);
    if watcher_client {
        // Probe-style init (GP-1 s2 / PR #47), optionally with cache priming
        // disabled (the round-3 starvation fix under test).
        let mut init_options = serde_json::json!({"files": {"watcher": "client"}});
        if std::env::var("STRESS_NO_PRIME").as_deref() == Ok("1") {
            init_options["cachePriming"] = serde_json::json!({"enable": false});
        }
        session
            .initialize_with_options(
                &root_uri,
                Some(init_options),
                Some(serde_json::json!({
                    "workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}}
                })),
            )
            .expect("initialize (watcher=client)");
    } else {
        // Product-style init (app_lsp::startup_session — no watcher option).
        session.initialize(&root_uri).expect("initialize (plain)");
    }
    session.start_stderr_drain();
    session
}

fn dump_session(label: &str, session: &mut RustAnalyzerSession) {
    let stats = session.reader_stats();
    eprintln!(
        "[stress:{label}] reader stats: frames={} bytes={} terminal={:?} child_running={} exit_status={:?}",
        stats.frames_forwarded,
        stats.payload_bytes,
        stats.terminal,
        session.is_running(),
        session.exit_status_string()
    );
    let buffered = session.buffered_diagnostic_notifications();
    eprintln!(
        "[stress:{label}] buffered diagnostic notifications: {}",
        buffered.len()
    );
    for (i, n) in buffered.iter().enumerate() {
        eprintln!(
            "[stress:{label}] buffered[{i}] uri_hash={:?} diagnostics={} errors={}",
            n.uri_hash, n.diagnostic_count, n.error_count
        );
    }
    let ring = session.stderr_ring();
    if let Ok(guard) = ring.lock() {
        eprintln!("[stress:{label}] stderr ring ({} lines):", guard.len());
        for line in guard.iter() {
            eprintln!("[stress:{label}] {line}");
        }
    }
}

#[test]
#[ignore = "spawns real rust-analyzer processes and loops for minutes; run explicitly (PKT-S3-WEDGE-R3 reproduction harness)"]
fn two_ra_same_workspace_didchange_stress() {
    let cycles: u32 = std::env::var("STRESS_CYCLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(15);
    let solo = std::env::var("STRESS_SOLO").as_deref() == Ok("1");
    let deadline = Duration::from_secs(
        std::env::var("STRESS_DEADLINE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30),
    );

    let discovery = RustAnalyzerDiscovery {
        path_env: std::env::var("PATH").ok(),
        ..Default::default()
    };
    let DiscoveredBinary::Found { path: binary, .. } = discovery.resolve() else {
        eprintln!("[stress] rust-analyzer not found on PATH; skipping");
        return;
    };

    // Fresh temp workspace from the GP-1 fixture.
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp1-rust");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!("legion-ra-stress-{nanos}"));
    copy_dir_recursive(&fixture, &workspace).expect("copy fixture");

    // GP-1 parity: git-init + commit the workspace before rust-analyzer
    // launches (STRESS_NO_GIT=1 skips, for bisecting whether the git state
    // changes rust-analyzer behavior).
    if std::env::var("STRESS_NO_GIT").as_deref() != Ok("1") {
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.email", "stress@legion.test"],
            vec!["config", "user.name", "RA Stress"],
            vec!["add", "."],
            vec!["commit", "-m", "initial"],
        ] {
            let status = std::process::Command::new("git")
                .args(&args)
                .current_dir(&workspace)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .expect("run git");
            assert!(status.success(), "git {args:?} failed");
        }
    }

    let scratchpad_path = workspace.join("src").join("scratchpad.rs");
    let scratchpad_uri = path_to_file_uri(&scratchpad_path);
    let main_uri = path_to_file_uri(&workspace.join("src").join("main.rs"));
    let at_rest = fs::read_to_string(&scratchpad_path).expect("read scratchpad");
    let main_at_rest =
        fs::read_to_string(workspace.join("src").join("main.rs")).expect("read main.rs");

    eprintln!(
        "[stress] workspace={} cycles={cycles} solo={solo}",
        workspace.display()
    );

    // Session A: the probe (watcher=client). Session B: the product shape.
    let mut a = spawn_session(&workspace, &binary, true);
    let mut b = (!solo).then(|| spawn_session(&workspace, &binary, false));

    a.did_open(&scratchpad_uri, "rust", 1, &at_rest)
        .expect("A did_open");
    if let Some(b) = b.as_mut() {
        b.did_open(&main_uri, "rust", 1, &main_at_rest)
            .expect("B did_open");
    }

    // Let both settle on the initial state.
    let initial = a.pump_diagnostics(&scratchpad_uri, Duration::from_secs(30));
    eprintln!(
        "[stress] A initial pump: notifications_for_uri={}",
        initial.len()
    );
    if initial.is_empty() {
        // URI-echo forensics: rust-analyzer may republish under a normalized
        // URI form; find which candidate form matches what it actually sent.
        eprintln!(
            "[stress] our uri: {scratchpad_uri} hash={:?}",
            legion_lsp::lsp_diagnostic_uri_fingerprint(&scratchpad_uri)
        );
        let lower_drive = scratchpad_uri.replacen("file:///C:", "file:///c:", 1);
        let pct = scratchpad_uri.replacen("file:///C:", "file:///C%3A", 1);
        let lower_pct = scratchpad_uri.replacen("file:///C:", "file:///c%3A", 1);
        for (name, candidate) in [
            ("lowercase-drive", &lower_drive),
            ("percent-colon", &pct),
            ("lowercase+percent", &lower_pct),
        ] {
            eprintln!(
                "[stress] candidate {name}: {candidate} hash={:?}",
                legion_lsp::lsp_diagnostic_uri_fingerprint(candidate)
            );
        }
    }

    let mut version: i64 = 1;
    for cycle in 1..=cycles {
        // Error phase.
        version += 1;
        let error_text = ERROR_TEXT_TEMPLATE.replace("{N}", &cycle.to_string());
        if let Err(e) = a.did_change(&scratchpad_uri, version, &error_text) {
            dump_session("A", &mut a);
            if let Some(b) = b.as_mut() {
                dump_session("B", b);
            }
            panic!("cycle {cycle}: A did_change(error) failed: {e}");
        }
        // B churn: version-bumped re-send of main.rs with a cycle comment.
        if let Some(b) = b.as_mut() {
            let churn = format!("{main_at_rest}// churn {cycle}\n");
            let _ = b.did_change(&main_uri, version, &churn);
        }
        if !a.pump_until_has_error_for(&scratchpad_uri, deadline) {
            eprintln!("[stress] cycle {cycle}: WEDGE (error phase) — post-mortem:");
            dump_session("A", &mut a);
            if let Some(b) = b.as_mut() {
                dump_session("B", b);
            }
            panic!("cycle {cycle}: no error diagnostic within 30s — wedge reproduced");
        }

        // Clear phase.
        version += 1;
        let fixed_text = FIXED_TEXT_TEMPLATE.replace("{N}", &cycle.to_string());
        if let Err(e) = a.did_change(&scratchpad_uri, version, &fixed_text) {
            dump_session("A", &mut a);
            if let Some(b) = b.as_mut() {
                dump_session("B", b);
            }
            panic!("cycle {cycle}: A did_change(fix) failed: {e}");
        }
        if !a.pump_until_diagnostics_clear(&scratchpad_uri, deadline) {
            eprintln!("[stress] cycle {cycle}: WEDGE (clear phase) — post-mortem:");
            dump_session("A", &mut a);
            if let Some(b) = b.as_mut() {
                dump_session("B", b);
            }
            panic!("cycle {cycle}: diagnostics did not clear within 30s — wedge reproduced");
        }
        eprintln!("[stress] cycle {cycle}/{cycles} ok");
    }

    assert!(
        a.is_running(),
        "A must still be alive after {cycles} cycles"
    );
    if let Some(b) = b.as_mut() {
        assert!(
            b.is_running(),
            "B must still be alive after {cycles} cycles"
        );
    }

    let _ = fs::remove_dir_all(&workspace);
}
