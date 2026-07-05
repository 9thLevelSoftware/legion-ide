//! GP-1 Golden Path smoke runner (M8 milestone closer).
//!
//! Invoked by `cargo run -p xtask -- golden-path-1` (subprocess model — xtask
//! cannot depend on legion-app, so it spawns this binary and reads its exit
//! code + the evidence TOML).
//!
//! # Steps
//! s1 copy-fixture: copy fixture to temp dir; git-init; open as Trusted workspace.
//! s2 lsp-ready:    start rust-analyzer session, pump until initialized (SKIP if absent).
//! s3 diagnostics:  introduce compile error via app-edit+did_change; pump until error;
//!                  fix; pump until clear.
//! s4 search:       workspace search for known literal; assert hit count; test case option.
//! s5 terminal:     launch trusted terminal; run `cargo test`; poll exit marker (SKIP if no PTY).
//! s6 git:          app-edit + save; RefreshGit (dirty); StageGitHunk; CommitGitChanges; assert clean.
//! s7 evidence:     write `target/golden-path/gp1_report.toml`; optionally also write under
//!                  plans/evidence/production/M8/ when --record-evidence is passed.
//!
//! # Constraints
//! - Never writes inside the Legion repo (except target/ and --record-evidence path).
//! - Fixture copies live in OS temp; cleaned on success, left on failure.
//! - CARGO_BUILD_JOBS=4 is set by the xtask caller; not enforced here.
//! - Zero egress: all operations are local.

use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use legion_app::{
    AppCommandOutcome, AppComposition,
    language::{
        DiscoveredBinary, RustAnalyzerDiscovery, RustAnalyzerLaunchConfig, RustAnalyzerSession,
    },
};
use legion_editor::{TextEdit, TextPosition, TextRange};
use legion_lsp::{LspServerProcessConfig, LspStdioLauncher, LspSupervisorConfig};
use legion_protocol::{
    BufferId, CapabilityDecisionId, CapabilityId, CausalityId, CorrelationId, FileFingerprint,
    LanguageId, LanguageServerId, LspConfiguredServerIdentity, LspLaunchPolicyDecision,
    LspWorkspaceTrustPosture, PrincipalId, RedactionHint, SemanticPrivacyScope,
    TerminalPanelStatusKind, TerminalSessionId, WorkspaceId, WorkspaceRootId, WorkspaceTrustState,
};
use legion_ui::{CommandDispatchIntent, GitHunkStageProjection, SearchScopeProjection};
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Step status
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum StepStatus {
    Passed,
    Failed,
    Skipped,
}

impl StepStatus {
    fn as_str(&self) -> &'static str {
        match self {
            StepStatus::Passed => "passed",
            StepStatus::Failed => "failed",
            StepStatus::Skipped => "skipped",
        }
    }
}

struct StepRecord {
    id: &'static str,
    started_utc: String,
    finished_utc: String,
    duration_ms: u128,
    status: StepStatus,
    detail: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// CLI args
// ─────────────────────────────────────────────────────────────────────────────

struct Args {
    fixture_dir: PathBuf,
    out_dir: PathBuf,
    evidence_dir: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut fixture_dir: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut evidence_dir: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture-dir" => {
                i += 1;
                fixture_dir = Some(PathBuf::from(
                    args.get(i).ok_or("--fixture-dir needs value")?,
                ));
            }
            "--out-dir" => {
                i += 1;
                out_dir = Some(PathBuf::from(args.get(i).ok_or("--out-dir needs value")?));
            }
            "--record-evidence" => {
                i += 1;
                evidence_dir = Some(PathBuf::from(
                    args.get(i).ok_or("--record-evidence needs value")?,
                ));
            }
            _ => {}
        }
        i += 1;
    }
    Ok(Args {
        fixture_dir: fixture_dir.ok_or("--fixture-dir required")?,
        out_dir: out_dir.unwrap_or_else(|| PathBuf::from("target/golden-path")),
        evidence_dir,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Convert Unix epoch seconds to an RFC 3339 UTC timestamp string.
///
/// Uses the civil-from-days algorithm by Howard Hinnant so that no external
/// date/time dependency is required.
fn epoch_secs_to_rfc3339(secs: u64) -> String {
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (year, month, day) = days_to_ymd(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Convert days since Unix epoch (1970-01-01) to a Gregorian (year, month, day) triple.
///
/// Algorithm: civil_from_days — Howard Hinnant, https://howardhinnant.github.io/date_algorithms.html
fn days_to_ymd(days: i64) -> (u32, u32, u32) {
    let z = days + 719468; // shift epoch to 0000-03-01
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month prime [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let mon = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = yoe as i64 + era * 400;
    let y = if mon <= 2 { y + 1 } else { y };
    (y as u32, mon as u32, d as u32)
}

fn utc_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    epoch_secs_to_rfc3339(now.as_secs())
}

fn run_timer<F, T>(f: F) -> (T, u128)
where
    F: FnOnce() -> T,
{
    let start = Instant::now();
    let result = f();
    (result, start.elapsed().as_millis())
}

/// Copy a directory tree (shallow: files only, no nested dirs beyond one level).
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("create dir {}: {e}", dst.display()))?;
    for entry in fs::read_dir(src).map_err(|e| format!("read dir {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        let ft = entry.file_type().map_err(|e| format!("file type: {e}"))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ft.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if ft.is_file() {
            fs::copy(&src_path, &dst_path).map_err(|e| {
                format!("copy {} -> {}: {e}", src_path.display(), dst_path.display())
            })?;
        }
    }
    Ok(())
}

fn git_cmd(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git {:?} spawn failed: {e}", args))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "git {:?} failed ({}): {}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn resolve_legion_git_sha(workspace_root: &Path) -> String {
    git_cmd(workspace_root, &["rev-parse", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn path_to_file_uri(path: &Path) -> String {
    let s = path.to_string_lossy();
    let forward = s.replace('\\', "/");
    if forward.starts_with('/') {
        // Unix-style absolute path.
        format!("file://{forward}")
    } else {
        // Windows path: lowercase the drive letter to match rust-analyzer's
        // URI normalization (rust-analyzer emits publishDiagnostics URIs with
        // lowercase drive letters on Windows, e.g. "file:///c:/...").
        let normalized = if forward.len() >= 2 && forward.as_bytes()[1] == b':' {
            let drive = forward[..1].to_ascii_lowercase();
            format!("{drive}{}", &forward[1..])
        } else {
            forward.to_string()
        };
        format!("file:///{normalized}")
    }
}

fn launch_policy_for_smoke(command: &str) -> LspLaunchPolicyDecision {
    LspLaunchPolicyDecision::evaluate(
        LspConfiguredServerIdentity {
            server_id: LanguageServerId(901),
            workspace_id: WorkspaceId(9),
            root_id: Some(WorkspaceRootId(9)),
            language_id: LanguageId("rust".to_string()),
            display_name: "rust-analyzer-gp1-smoke".to_string(),
            command_hash: FileFingerprint {
                algorithm: "gp1-smoke".to_string(),
                value: command.to_string(),
            },
            args_hash: None,
            env_hash: None,
            cwd_hash: None,
            settings_hash: None,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        LspWorkspaceTrustPosture {
            workspace_id: WorkspaceId(9),
            workspace_trust_state: WorkspaceTrustState::Trusted,
            privacy_scope: SemanticPrivacyScope::Workspace,
            privacy_scope_allowed: true,
            required_capability: CapabilityId("process.spawn".to_string()),
            decision_id: Some(CapabilityDecisionId(9)),
            diagnostics: Vec::new(),
            schema_version: 1,
        },
        true,
        CorrelationId(9),
        CausalityId(Uuid::from_u128(9001)),
        Vec::new(),
        1,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Terminal polling
// ─────────────────────────────────────────────────────────────────────────────

fn poll_terminal_for_marker(
    app: &mut AppComposition,
    session_id: TerminalSessionId,
    marker: &str,
    deadline: Instant,
) -> Option<String> {
    let mut last_row_count = 0usize;
    let mut last_status = TerminalPanelStatusKind::Running;
    while let Ok(AppCommandOutcome::TerminalPanelUpdated(projection)) =
        app.dispatch_ui_intent(CommandDispatchIntent::TerminalOutputPoll { session_id })
    {
        // Log whenever rows are added so we can see output accumulating.
        let row_count = projection.output_rows.len();
        if row_count != last_row_count || projection.status.kind != last_status {
            eprintln!(
                "[s5-poll] rows={} status={:?} elapsed={}ms",
                row_count,
                projection.status.kind,
                deadline
                    .checked_duration_since(Instant::now())
                    .map(|r| TERMINAL_POLL_DEADLINE_SECS * 1000 - r.as_millis() as u64)
                    .unwrap_or(TERMINAL_POLL_DEADLINE_SECS * 1000)
            );
            last_row_count = row_count;
            last_status = projection.status.kind;
        }
        // Scan the accumulated scrollback for the exit marker.
        for row in &projection.output_rows {
            if row.redacted_payload.contains(marker) {
                return Some(row.redacted_payload.clone());
            }
        }
        // Stop as soon as the session is definitively done — no new rows will
        // arrive once the session is Exited, Crashed, or Failed.  Without this
        // early break the loop would spin until the full 120 s deadline after
        // the session's own 30 s internal deadline kills it.
        let session_done = matches!(
            projection.status.kind,
            TerminalPanelStatusKind::Exited
                | TerminalPanelStatusKind::Failed
                | TerminalPanelStatusKind::Crashed
        );
        if session_done || Instant::now() >= deadline {
            // Dump accumulated rows for post-mortem diagnostics.  Full
            // scrollback, full payloads: rows are already capped by the
            // product's per-row limit, and a failed s5 has at most a few
            // dozen rows — completeness beats brevity when the only
            // diagnostics channel is a CI log (LSP-B lesson).
            eprintln!("[s5-poll] loop exit: session_done={session_done} rows={row_count}");
            for (i, row) in projection.output_rows.iter().enumerate() {
                eprintln!(
                    "[s5-poll] row[{i}] len={} truncated={} payload={:?}",
                    row.redacted_payload.len(),
                    row.truncated,
                    row.redacted_payload
                );
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s1: copy fixture + open workspace
// ─────────────────────────────────────────────────────────────────────────────

struct S1Result {
    temp_dir: PathBuf,
    app: AppComposition,
}

fn run_s1(fixture_dir: &Path) -> Result<S1Result, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir =
        std::env::temp_dir().join(format!("legion-gp1-smoke-{}-{}", process::id(), nanos));

    copy_dir_recursive(fixture_dir, &temp_dir)?;

    // git init in the temp dir
    git_cmd(&temp_dir, &["init", "-b", "main"])?;
    git_cmd(
        &temp_dir,
        &["config", "user.email", "gp1-smoke@legion.test"],
    )?;
    git_cmd(&temp_dir, &["config", "user.name", "GP-1 Smoke"])?;
    git_cmd(&temp_dir, &["add", "."])?;
    git_cmd(
        &temp_dir,
        &["commit", "-m", "initial: smoke fixture baseline"],
    )?;

    let mut app = AppComposition::new();
    app.open_workspace(
        &temp_dir,
        WorkspaceTrustState::Trusted,
        PrincipalId("gp1-smoke".to_string()),
    )
    .map_err(|e| format!("open_workspace failed: {e:?}"))?;

    Ok(S1Result { temp_dir, app })
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s2: start rust-analyzer session
// ─────────────────────────────────────────────────────────────────────────────

fn run_s2(temp_dir: &Path) -> Result<Option<RustAnalyzerSession>, String> {
    let discovery = RustAnalyzerDiscovery {
        path_env: std::env::var("PATH").ok(),
        ..Default::default()
    };
    let binary_path = match discovery.resolve() {
        DiscoveredBinary::Found { path, .. } => path,
        DiscoveredBinary::NotFound => return Ok(None),
    };

    let version = RustAnalyzerDiscovery::probe_version(&binary_path);
    eprintln!(
        "[s2] rust-analyzer: {} version={:?}",
        binary_path.display(),
        version
    );

    let command = binary_path.to_string_lossy().into_owned();
    let config = RustAnalyzerLaunchConfig {
        discovery: RustAnalyzerDiscovery {
            configured_path: Some(binary_path.clone()),
            ..Default::default()
        },
        supervisor: LspSupervisorConfig {
            launch_policy: launch_policy_for_smoke(&command),
            process: LspServerProcessConfig {
                command: command.clone(),
                args: Vec::new(),
                cwd: Some(temp_dir.to_path_buf()),
                env: Vec::new(),
            },
            initial_backoff_ms: 50,
            max_backoff_ms: 1000,
            max_restart_attempts: 1,
        },
        server_id: LanguageServerId(901),
        language_id: LanguageId("rust".to_string()),
    };

    let mut launcher = LspStdioLauncher::new();
    let mut session =
        RustAnalyzerSession::launch(config, &mut launcher).map_err(|e| format!("launch: {e}"))?;

    let root_uri = path_to_file_uri(temp_dir);
    session
        .initialize(&root_uri)
        .map_err(|e| format!("initialize: {e}"))?;

    eprintln!(
        "[s2] session initialized; health={:?}",
        session.health().init_status
    );

    Ok(Some(session))
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s3: diagnostics cycle
// ─────────────────────────────────────────────────────────────────────────────

const SCRATCHPAD_ERROR_TEXT: &str = r#"// scratchpad module — smoke-introduced compile error
pub fn scratchpad() {
    // type mismatch: &str assigned to i32
    let _x: i32 = "smoke_type_error";
}
"#;

/// Content used for the "fix" phase of the s3 diagnostics cycle.
///
/// This is intentionally DIFFERENT from the at-rest fixture content so that
/// rust-analyzer does not deduplicate the `publishDiagnostics` notification.
/// RA tracks previously-seen file content per URI: if the fix content equals
/// the at-rest content it already analysed during `didOpen` (version 1), RA
/// may skip the re-analysis and never re-emit the "no errors" notification.
/// Using a distinct but still-valid Rust snippet forces a fresh analysis.
const SCRATCHPAD_FIXED_TEXT: &str = "// scratchpad module — smoke-fixed\npub fn scratchpad() {}\n";

fn run_s3(
    temp_dir: &Path,
    app: &mut AppComposition,
    session: &mut RustAnalyzerSession,
) -> Result<(), String> {
    let scratchpad_path = temp_dir.join("src").join("scratchpad.rs");
    let scratchpad_uri = path_to_file_uri(&scratchpad_path);

    // Open scratchpad.rs in the editor via app path.
    let scratchpad_str = scratchpad_path.to_string_lossy().into_owned();
    let _ = app
        .open_file(&scratchpad_str)
        .map_err(|e| format!("open scratchpad.rs: {e:?}"))?;

    // Capture the active buffer id — needed to project diagnostics through
    // AppComposition::ingest_lsp_publish_diagnostics_for_buffer (I-1).
    let buffer_id: BufferId = app
        .active_buffer_id()
        .ok_or("s3: no active buffer after open_file(scratchpad.rs)")?;

    // Read the at-rest content and send did_open.
    let at_rest_text =
        fs::read_to_string(&scratchpad_path).map_err(|e| format!("read scratchpad: {e}"))?;
    session
        .did_open(&scratchpad_uri, "rust", 1, &at_rest_text)
        .map_err(|e| format!("did_open: {e}"))?;

    // Initial pump: let rust-analyzer settle (bounded 30s).
    eprintln!("[s3] initial pump (up to 30s) ...");
    let initial_pump_started = Instant::now();
    let initial = session.pump_diagnostics(&scratchpad_uri, Duration::from_secs(30));
    eprintln!(
        "[s3] initial pump done: notifications_for_uri={} elapsed={}ms",
        initial.len(),
        initial_pump_started.elapsed().as_millis()
    );

    // --- introduce compile error ---
    eprintln!("[s3] introducing compile error via app edit path ...");

    // Edit via AppComposition (app-edit path).
    let lines: Vec<&str> = at_rest_text.lines().collect();
    let last_line = lines.len().saturating_sub(1);
    let last_col = lines.last().map(|l| l.len()).unwrap_or(0);
    let error_edit = TextEdit::new(
        TextRange::new(
            TextPosition::new(0, 0),
            TextPosition::new(last_line, last_col),
        ),
        SCRATCHPAD_ERROR_TEXT,
    );
    let _ = app
        .edit_active_buffer(error_edit)
        .map_err(|e| format!("edit_active_buffer (error): {e:?}"))?;

    // Sync edit to rust-analyzer via did_change.
    //
    // We intentionally do NOT write ERROR_TEXT to disk before sending
    // did_change.  Writing the file before didChange triggers RA's FS-watcher,
    // which on Windows can enter a race that leaves RA's internal document
    // state inconsistent.  Specifically: if RA processes the FS event at a
    // point when its in-memory version and the didChange version conflict,
    // RA may silently stop responding to subsequent didChange messages (C and
    // D notifications are never sent for did_change(3)).  Omitting the disk
    // write keeps RA's in-memory model as the authoritative source and avoids
    // this race entirely.  The error pump (pump_until_has_error_for) already
    // handles the initial clearing ack by skipping it and waiting for a
    // notification with error_count > 0.
    let mut doc_version: i64 = 2;
    session
        .did_change(&scratchpad_uri, doc_version, SCRATCHPAD_ERROR_TEXT)
        .map_err(|e| format!("did_change (error): {e}"))?;

    // Pump until rust-analyzer reports an error diagnostic for the erroneous
    // content.  rust-analyzer often emits a "clear" notification (error_count=0)
    // immediately after didChange as an acknowledgement before re-analysing; we
    // skip that with pump_until_has_error_for which only returns true on the
    // first notification that has error_count > 0.
    //
    // The pump runs in bounded slices with a version-bumped re-send between
    // slices. Evidence (campaign ledger 2026-07-05; 4 occurrences across
    // Windows local + ubuntu CI run 28747873556): rust-analyzer occasionally
    // goes COMPLETELY silent after a didChange that lands while its initial
    // prime-caches pass is still running — the didOpen publish arrives
    // (~1s), then zero notifications for ANY uri for 120s+ (post-mortem:
    // buffered_notifications=0, health Fresh, restart_count=0, send
    // succeeded, reader thread live). The stall is inside RA's analysis
    // queue, not our transport. Editors unstick it the way a user does —
    // another keystroke ⇒ another didChange with a bumped version.
    // Re-sending the SAME content still publishes: RA's dedup compares
    // against the last PUBLISHED set (the clean v1 set), not the last
    // analysed text. A genuinely dead server still fails the overall
    // deadline. Root-cause confirmation via RA stderr lands with the LSP-C
    // stderr ring buffer.
    eprintln!("[s3] pumping for error diagnostic (up to 120s, nudge every 30s) ...");
    let mut got_error = false;
    for slice in 0..4u32 {
        if slice > 0 {
            doc_version += 1;
            eprintln!(
                "[s3] no diagnostics after {}s; nudging rust-analyzer with did_change v{doc_version} (silent-stall workaround)",
                slice * 30
            );
            session
                .did_change(&scratchpad_uri, doc_version, SCRATCHPAD_ERROR_TEXT)
                .map_err(|e| format!("did_change (error nudge): {e}"))?;
        }
        if session.pump_until_has_error_for(&scratchpad_uri, Duration::from_secs(30)) {
            got_error = true;
            break;
        }
    }
    eprintln!("[s3] error diagnostic received: {got_error}");
    if !got_error {
        // Post-mortem: distinguish "rust-analyzer silent for the whole
        // wait" (starvation / stall) from "notifications arrived but never
        // matched the error predicate" (product-side race or fingerprint
        // mismatch). Flake observed locally under concurrent builds and on
        // ubuntu CI (run 28747873556).
        let expected_hash = legion_lsp::lsp_diagnostic_uri_fingerprint(&scratchpad_uri);
        let buffered = session.buffered_diagnostic_notifications();
        eprintln!(
            "[s3] POST-MORTEM: expected uri_hash={expected_hash:?} buffered_notifications={}",
            buffered.len()
        );
        for (i, n) in buffered.iter().enumerate() {
            eprintln!(
                "[s3] buffered[{i}] uri_hash={:?} match={} diagnostics={} errors={} warnings={}",
                n.uri_hash,
                n.uri_hash == expected_hash,
                n.diagnostic_count,
                n.error_count,
                n.warning_count
            );
        }
        eprintln!("[s3] health: {:?}", session.health());
        return Err(
            "s3: expected >=1 error diagnostic after introducing type mismatch; \
             none arrived within 120s deadline"
                .to_string(),
        );
    }

    // Prove the error appears through AppComposition's projection layer (I-1).
    // Route the raw publishDiagnostics payload through the product path the
    // desktop uses: ingest_lsp_publish_diagnostics_for_buffer → LanguageToolingProjection.
    eprintln!("[s3] asserting error through AppComposition projection ...");
    let error_raw = session
        .take_last_diagnostic_params_for(&scratchpad_uri)
        .ok_or("s3: raw publishDiagnostics params absent after pump_until_has_error_for")?;
    let error_projection = app
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &error_raw, false, None)
        .map_err(|e| format!("s3: ingest_lsp_publish_diagnostics_for_buffer (error): {e:?}"))?;
    eprintln!(
        "[s3] projection.problems (error phase): {}",
        error_projection.problems.len()
    );
    if error_projection.problems.is_empty() {
        return Err(
            "s3: LanguageToolingProjection.problems is empty after error ingested; \
             expected >=1 — projection layer not seeing the diagnostic"
                .to_string(),
        );
    }

    // --- fix the error ---
    eprintln!("[s3] fixing compile error via app edit path ...");

    let error_lines: Vec<&str> = SCRATCHPAD_ERROR_TEXT.lines().collect();
    let error_last_line = error_lines.len().saturating_sub(1);
    let error_last_col = error_lines.last().map(|l| l.len()).unwrap_or(0);
    let fix_edit = TextEdit::new(
        TextRange::new(
            TextPosition::new(0, 0),
            TextPosition::new(error_last_line, error_last_col),
        ),
        SCRATCHPAD_FIXED_TEXT,
    );
    let _ = app
        .edit_active_buffer(fix_edit)
        .map_err(|e| format!("edit_active_buffer (fix): {e:?}"))?;

    // Sync fix to rust-analyzer with the distinct FIXED content.
    // Using SCRATCHPAD_FIXED_TEXT (≠ at_rest_text) prevents RA from
    // de-duplicating the re-analysis result against the at_rest_text it
    // already analysed during did_open (version 1).
    //
    // We do NOT write to disk before sending did_change here: writing
    // at_rest_text to disk before the fix did_change causes RA's FS-watcher
    // to fire, publishing a 0-error notification E.  RA then deduplicates
    // the fix did_change's analysis result (also 0 errors) against E and
    // skips the publishDiagnostics, so the clear pump sees nothing and
    // times out.
    doc_version += 1;
    session
        .did_change(&scratchpad_uri, doc_version, SCRATCHPAD_FIXED_TEXT)
        .map_err(|e| format!("did_change (fix): {e}"))?;

    // Pump until errors are clear (bounded 60s), with the same sliced
    // nudge as the error phase — the silent-stall race applies to any
    // didChange (see the error-pump comment above).
    eprintln!("[s3] pumping until errors clear (up to 60s, nudge at 30s) ...");
    let mut cleared = false;
    for slice in 0..2u32 {
        if slice > 0 {
            doc_version += 1;
            eprintln!(
                "[s3] errors not cleared after 30s; nudging rust-analyzer with did_change v{doc_version} (silent-stall workaround)"
            );
            session
                .did_change(&scratchpad_uri, doc_version, SCRATCHPAD_FIXED_TEXT)
                .map_err(|e| format!("did_change (fix nudge): {e}"))?;
        }
        if session.pump_until_diagnostics_clear(&scratchpad_uri, Duration::from_secs(30)) {
            cleared = true;
            break;
        }
    }
    eprintln!("[s3] errors cleared: {cleared}");

    // Write the at-rest content to disk after the pump completes (success or
    // failure) so the fixture is in a clean, committed state for s6's
    // git-clean check.  We never wrote ERROR_TEXT to disk, so the file still
    // holds its original at_rest_text — but we write it explicitly here to
    // ensure idempotency and to make s6's expectations clear regardless of
    // future edits to this function.
    fs::write(&scratchpad_path, &at_rest_text)
        .map_err(|e| format!("write at-rest content to scratchpad: {e}"))?;

    if !cleared {
        return Err(
            "s3: error diagnostics did not clear after fix within 60s deadline".to_string(),
        );
    }

    // Prove the clear is visible through AppComposition's projection layer (I-1).
    eprintln!("[s3] asserting clear through AppComposition projection ...");
    let clear_raw = session
        .take_last_diagnostic_params_for(&scratchpad_uri)
        .ok_or("s3: raw publishDiagnostics params absent after pump_until_diagnostics_clear")?;
    let clear_projection = app
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &clear_raw, false, None)
        .map_err(|e| format!("s3: ingest_lsp_publish_diagnostics_for_buffer (clear): {e:?}"))?;
    eprintln!(
        "[s3] projection.problems (clear phase): {}",
        clear_projection.problems.len()
    );
    if !clear_projection.problems.is_empty() {
        return Err(format!(
            "s3: LanguageToolingProjection.problems has {} item(s) after error fixed; \
             expected 0 -- projection layer not clearing correctly",
            clear_projection.problems.len()
        ));
    }

    eprintln!(
        "[s3] diagnostics cycle complete (error introduced, projected, fixed, projection cleared)"
    );
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s4: workspace search
// ─────────────────────────────────────────────────────────────────────────────

fn run_s4(app: &mut AppComposition) -> Result<(), String> {
    const MARKER: &str = "SMOKE_MARKER_ALPHA";

    // Open main.rs so the workspace has an active file for the search.
    // (The workspace search is not scoped to the active file, but we need at
    // least one buffer open for workspace context.)
    eprintln!("[s4] running workspace search for '{MARKER}' ...");
    let search_result = app
        .dispatch_ui_intent(CommandDispatchIntent::RunSearch {
            scope: SearchScopeProjection::Workspace,
            query: MARKER.to_string(),
            limit: 50,
            case_sensitive: None,
            whole_word: None,
            use_regex: None,
        })
        .map_err(|e| format!("search dispatch: {e:?}"))?;
    let projection = match search_result {
        AppCommandOutcome::SearchUpdated(p) => p,
        other => return Err(format!("s4: expected SearchUpdated, got {other:?}")),
    };
    let hit_count = projection.results.len();
    eprintln!("[s4] search results: {hit_count}");
    if hit_count == 0 {
        return Err(format!(
            "s4: expected >=1 result for '{MARKER}' in workspace; got 0 (status={:?})",
            projection.status.kind
        ));
    }
    // M-2: bound the count tightly — the fixture is a 2-file project; the marker
    // appears only in main.rs so the result set should be small.
    if hit_count > 5 {
        return Err(format!(
            "s4: suspiciously large hit count for '{MARKER}'; got {hit_count}, expected <=5 — \
             fixture may have been modified or the search is matching unintended files"
        ));
    }

    // M-1: case-sensitivity proof — lowercase "smoke_marker_alpha" must return 0 hits
    // because the fixture marker is ALL CAPS.  Without this check a case-insensitive
    // search engine would pass the upper-case query yet silently undercount nothing.
    let cs_lower_query = MARKER.to_ascii_lowercase(); // "smoke_marker_alpha"
    eprintln!(
        "[s4] case-sensitivity proof: searching lowercase '{cs_lower_query}' (expected 0) ..."
    );
    let cs_lower_result = app
        .dispatch_ui_intent(CommandDispatchIntent::RunSearch {
            scope: SearchScopeProjection::Workspace,
            query: cs_lower_query.clone(),
            limit: 50,
            // Explicit end-to-end exercise of the WS-SEARCH-01 option
            // threading (stronger than relying on the default).
            case_sensitive: Some(true),
            whole_word: None,
            use_regex: None,
        })
        .map_err(|e| format!("cs-lower search dispatch: {e:?}"))?;
    let cs_lower_projection = match cs_lower_result {
        AppCommandOutcome::SearchUpdated(p) => p,
        other => {
            return Err(format!(
                "s4: expected SearchUpdated for cs-lower, got {other:?}"
            ));
        }
    };
    let cs_lower_count = cs_lower_projection.results.len();
    eprintln!("[s4] case-sensitive lowercase results: {cs_lower_count} (expected 0)");
    if cs_lower_count != 0 {
        return Err(format!(
            "s4: case-sensitive search for '{cs_lower_query}' returned {cs_lower_count} hit(s); \
             expected 0 — marker is ALL-CAPS in fixture, search must be case-sensitive by default"
        ));
    }

    // Exercise case-insensitive option: "nocase" prefix makes it case-insensitive.
    // Verify we also get hits when we explicitly opt into case-insensitive mode.
    let nocase_query = format!("nocase {}", MARKER.to_ascii_lowercase());
    eprintln!("[s4] running case-insensitive search: '{nocase_query}' ...");
    let nocase_result = app
        .dispatch_ui_intent(CommandDispatchIntent::RunSearch {
            scope: SearchScopeProjection::Workspace,
            query: nocase_query.clone(),
            limit: 50,
            case_sensitive: Some(false),
            whole_word: None,
            use_regex: None,
        })
        .map_err(|e| format!("nocase search dispatch: {e:?}"))?;
    let nocase_projection = match nocase_result {
        AppCommandOutcome::SearchUpdated(p) => p,
        other => {
            return Err(format!(
                "s4: expected SearchUpdated for nocase, got {other:?}"
            ));
        }
    };
    let nocase_count = nocase_projection.results.len();
    eprintln!("[s4] nocase search results: {nocase_count}");
    if nocase_count == 0 {
        return Err(format!(
            "s4: expected >=1 result for nocase query '{nocase_query}'; got 0"
        ));
    }

    eprintln!(
        "[s4] search step passed (hits={hit_count}, cs_lower_hits={cs_lower_count}, nocase_hits={nocase_count})"
    );
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s5: terminal
// ─────────────────────────────────────────────────────────────────────────────

const TERMINAL_POLL_DEADLINE_SECS: u64 = 120;
const SMOKE_EXIT_MARKER: &str = "SMOKE_EXIT:";

fn run_s5(temp_dir: &Path, app: &mut AppComposition) -> Result<Option<String>, String> {
    // Pre-warm: compile the fixture test binary via a subprocess so the
    // terminal step (which has a hard 30-second session deadline enforced by
    // AppComposition) only needs to *run* the pre-built binary rather than
    // compile it.  The fixture has no external deps so compilation is fast,
    // but even a few-second compilation on a cold cache would otherwise push
    // the total time past the session deadline, killing the session before
    // `echo SMOKE_EXIT:0` is written.
    eprintln!("[s5] pre-warming fixture test build (cargo test --no-run) ...");
    match std::process::Command::new("cargo")
        .args(["test", "--no-run", "--no-fail-fast"])
        .current_dir(temp_dir)
        .env("CARGO_BUILD_JOBS", "4")
        .status()
    {
        Err(e) => {
            // cargo not available — skip the entire step, same as no PTY.
            eprintln!("[s5] cargo not on PATH; SKIP: {e}");
            return Ok(Some(format!("cargo not available for pre-warm: {e}")));
        }
        Ok(status) if !status.success() => {
            return Err(format!(
                "s5: pre-warm 'cargo test --no-run' failed (exit={})",
                status.code().unwrap_or(-1)
            ));
        }
        Ok(_) => eprintln!("[s5] pre-warm complete; test binary cached"),
    }

    // The workspace was opened as Trusted — the product gate auto-enables the
    // terminal runtime on an explicit TerminalLaunch intent.
    eprintln!("[s5] launching terminal (trusted workspace; product gate) ...");
    let launch_outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "gp1-smoke-cargo-test".to_string(),
            // 300 s gives CI cold-start plenty of headroom (reviewer item h).
            // The product default is 30 s; here we override for the smoke only.
            timeout_secs: Some(300),
        })
        .map_err(|e| format!("terminal launch: {e:?}"))?;
    let launch_projection = match launch_outcome {
        AppCommandOutcome::TerminalPanelUpdated(p) => p,
        other => return Err(format!("s5: expected TerminalPanelUpdated, got {other:?}")),
    };

    // Check if the terminal is running; skip gracefully if unavailable.
    if launch_projection.status.kind != TerminalPanelStatusKind::Running {
        let reason = launch_projection
            .last_denial
            .clone()
            .unwrap_or_else(|| format!("terminal status={:?}", launch_projection.status.kind));
        eprintln!("[s5] terminal not running — SKIP: {reason}");
        return Ok(Some(reason));
    }

    let session_id = launch_projection
        .active_session_id
        .ok_or("s5: terminal running but no active session id")?;

    // Build the command sent to the interactive terminal.
    //
    // We use a script file so the literal "SMOKE_EXIT:" string does NOT appear
    // in the command text sent to the shell.  Without this, the ConPTY echoes
    // the input back to the output pipe and the poll would match the command
    // echo rather than the actual command output (false positive in <100ms).
    //
    // On Windows the terminal shell is cmd.exe (launched as `cmd /Q /K`).
    // We run the batch via `call "path.bat"` rather than `cmd /c "path.bat"`:
    // CALL executes a batch file within the CURRENT cmd.exe process, so its
    // stdout goes directly to the ConPTY output pipe — no nested process and
    // no console-handle inheritance ambiguity.
    //
    // Cargo flags:
    //   -q           suppress per-test status lines (only show summary)
    //   --color=never strip ANSI escape codes (they inflate character count)
    // The product splits output chunks into per-line rows and caps each ROW
    // at 240 chars, so the marker only needs its own short line; the flags
    // keep individual lines short and the scrollback quiet.
    // Write the runner script into the OS temp dir (NOT the fixture workspace git
    // root) so it never shows up as an untracked file during the s6 git step (I-2).
    let temp_str = temp_dir.to_string_lossy();
    let pid = process::id();
    let (script_path, terminal_cmd) = if cfg!(windows) {
        let bat_path = std::env::temp_dir().join(format!("gp1_smoke_test_{pid}.bat"));
        // Use \r\n line endings — cmd.exe in ConPTY requires CRLF.
        // Use /D in `cd` to change drive if temp is on a different drive.
        let bat_content = format!(
            "@echo off\r\ncd /d \"{temp_str}\"\r\ncargo test -q --no-fail-fast --color=never\r\necho SMOKE_EXIT:%ERRORLEVEL%\r\n"
        );
        fs::write(&bat_path, bat_content.as_bytes())
            .map_err(|e| format!("s5: write script: {e}"))?;
        // `call "path"` runs the batch in the current cmd.exe process.
        // The batch must be given with \r\n at end so cmd.exe processes it.
        let cmd = format!("call \"{}\"\r\n", bat_path.to_string_lossy());
        (bat_path, cmd)
    } else {
        let sh_path = std::env::temp_dir().join(format!("gp1_smoke_test_{pid}.sh"));
        // Sidecar transcript: ground truth that survives even when the PTY
        // scrollback projection loses rows (observed on macOS CI, run
        // 28741840232: block exited 0 in 173ms yet no script output row —
        // including the marker — ever appeared in the projection).  The
        // script records (1) whether/where cargo resolves and (2) the real
        // exit code, WITHOUT altering what flows to the PTY: cargo's
        // stdout/stderr and the marker echo reach the terminal exactly as
        // before.
        let sidecar = std::env::temp_dir().join(format!("gp1_smoke_sidecar_{pid}.txt"));
        let sidecar_str = sidecar.to_string_lossy();
        let sh_content = format!(
            "#!/bin/sh\n\
             command -v cargo > '{sidecar_str}' 2>&1\n\
             echo \"PROBE_CARGO_STATUS:$?\" >> '{sidecar_str}'\n\
             cd '{temp_str}'\n\
             cargo test -q --no-fail-fast --color=never\n\
             gp1_code=$?\n\
             echo \"SMOKE_EXIT:$gp1_code\" >> '{sidecar_str}'\n\
             echo \"SMOKE_EXIT:$gp1_code\"\n"
        );
        fs::write(&sh_path, sh_content.as_bytes()).map_err(|e| format!("s5: write script: {e}"))?;
        let cmd = format!("sh '{}'\n", sh_path.to_string_lossy());
        (sh_path, cmd)
    };
    eprintln!("[s5] runner script: {}", script_path.display());
    eprintln!(
        "[s5] sending command: {} ({} bytes)",
        terminal_cmd.trim(),
        terminal_cmd.len()
    );

    let _ = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalInput {
            session_id,
            payload: terminal_cmd,
        })
        .map_err(|e| format!("terminal input: {e:?}"))?;

    // Poll until the exit marker appears or deadline elapses.
    let deadline = Instant::now() + Duration::from_secs(TERMINAL_POLL_DEADLINE_SECS);
    eprintln!(
        "[s5] polling for '{SMOKE_EXIT_MARKER}' (up to {}s) ...",
        TERMINAL_POLL_DEADLINE_SECS
    );
    let hit = poll_terminal_for_marker(app, session_id, SMOKE_EXIT_MARKER, deadline);

    match hit {
        None => {
            // Post-mortem: dump the sidecar transcript (Unix script writes
            // it; absent on Windows or if the script never ran).  This is
            // the ground-truth channel when the PTY projection lost rows.
            let sidecar = std::env::temp_dir().join(format!("gp1_smoke_sidecar_{pid}.txt"));
            match fs::read_to_string(&sidecar) {
                Ok(text) => {
                    eprintln!("[s5] sidecar transcript ({}):", sidecar.display());
                    for line in text.lines() {
                        eprintln!("[s5-sidecar] {line}");
                    }
                }
                Err(e) => eprintln!(
                    "[s5] sidecar transcript unavailable ({}): {e}",
                    sidecar.display()
                ),
            }
            Err(format!(
                "s5: timeout ({TERMINAL_POLL_DEADLINE_SECS}s) waiting for '{SMOKE_EXIT_MARKER}' in terminal output"
            ))
        }
        Some(row) => {
            eprintln!("[s5] exit marker found in row: {row:?}");
            // Parse exit code from the marker payload (best-effort; redaction may truncate).
            let exit_ok = row.contains(&format!("{SMOKE_EXIT_MARKER}0"))
                || row.contains(&format!("{SMOKE_EXIT_MARKER} 0"));
            if !exit_ok {
                return Err(format!(
                    "s5: cargo test did not exit 0; marker row: {row:?}"
                ));
            }
            eprintln!("[s5] cargo test exited successfully");
            Ok(None) // not skipped
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s6: git workflow
// ─────────────────────────────────────────────────────────────────────────────

fn run_s6(temp_dir: &Path, app: &mut AppComposition) -> Result<(), String> {
    // Open src/main.rs and make an edit via the app save path.
    let main_rs = temp_dir.join("src").join("main.rs");
    let main_rs_str = main_rs.to_string_lossy().into_owned();

    // open_file returns FileId; the active buffer_id is available via active_buffer_id().
    app.open_file(&main_rs_str)
        .map_err(|e| format!("s6: open main.rs: {e:?}"))?;
    let buffer_id = app
        .active_buffer_id()
        .ok_or("s6: no active buffer after open_file")?;

    // Read buffer text to determine append position.
    let text = app
        .editor()
        .text(buffer_id)
        .map_err(|e| format!("s6: read buffer text: {e:?}"))?
        .to_string();
    let lines: Vec<&str> = text.lines().collect();
    let last_line = lines.len().saturating_sub(1);
    let last_col = lines.last().map(|l| l.len()).unwrap_or(0);

    // Append a smoke-edit comment.
    let append_edit = TextEdit::new(
        TextRange::new(
            TextPosition::new(last_line, last_col),
            TextPosition::new(last_line, last_col),
        ),
        "\n// smoke-edited-by-gp1\n",
    );
    app.edit_active_buffer(append_edit)
        .map_err(|e| format!("s6: edit_active_buffer: {e:?}"))?;
    app.save_active_buffer()
        .map_err(|e| format!("s6: save_active_buffer: {e:?}"))?;

    eprintln!("[s6] saved edit to src/main.rs; refreshing git projection ...");

    // RefreshGit — expect dirty file.
    let git_projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshGit)
        .map_err(|e| format!("s6: RefreshGit: {e:?}"))?
    {
        AppCommandOutcome::GitUpdated(p) => p,
        other => {
            return Err(format!(
                "s6: expected GitUpdated from RefreshGit, got {other:?}"
            ));
        }
    };

    if git_projection.changed_files.is_empty() {
        return Err(
            "s6: expected >=1 dirty file after save; git projection shows 0 changed files"
                .to_string(),
        );
    }
    eprintln!("[s6] dirty files: {}", git_projection.changed_files.len());

    // Find an unstaged hunk.
    let hunk = git_projection
        .hunks
        .iter()
        .find(|h| h.stage == GitHunkStageProjection::Unstaged)
        .ok_or("s6: expected >=1 unstaged hunk in git projection")?;
    let hunk_id = hunk.hunk_id.clone();
    eprintln!("[s6] staging hunk: {hunk_id}");

    // Stage the hunk.
    match app
        .dispatch_ui_intent(CommandDispatchIntent::StageGitHunk { hunk_id })
        .map_err(|e| format!("s6: StageGitHunk: {e:?}"))?
    {
        AppCommandOutcome::GitUpdated(_) => {}
        other => {
            return Err(format!(
                "s6: expected GitUpdated from StageGitHunk, got {other:?}"
            ));
        }
    };

    // Commit via app authority.
    eprintln!("[s6] committing via app authority ...");
    let committed = match app
        .dispatch_ui_intent(CommandDispatchIntent::CommitGitChanges {
            message: "smoke: gp1 git workflow verification".to_string(),
        })
        .map_err(|e| format!("s6: CommitGitChanges: {e:?}"))?
    {
        AppCommandOutcome::GitUpdated(p) => p,
        other => {
            return Err(format!(
                "s6: expected GitUpdated from CommitGitChanges, got {other:?}"
            ));
        }
    };
    eprintln!(
        "[s6] committed; post-commit changed_files={}",
        committed.changed_files.len()
    );

    // Assert the worktree is clean after commit (I-2).  Any remaining changed
    // files would indicate that the smoke left untracked or modified content
    // inside the fixture workspace root, which is a constraint violation.
    if !committed.changed_files.is_empty() {
        return Err(format!(
            "s6: worktree not clean after commit; {} changed file(s) still present — \
             smoke may have left artefacts inside the fixture git root",
            committed.changed_files.len()
        ));
    }

    // Verify git log shows our commit.
    let log = git_cmd(temp_dir, &["log", "-1", "--pretty=%s"])
        .map_err(|e| format!("s6: git log: {e}"))?;
    if !log.trim().contains("smoke: gp1 git workflow verification") {
        return Err(format!(
            "s6: expected commit message not found in git log; got: {log:?}"
        ));
    }

    eprintln!("[s6] git step passed");
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s7: write evidence TOML
// ─────────────────────────────────────────────────────────────────────────────

fn write_evidence(
    out_dir: &Path,
    evidence_dir: Option<&Path>,
    legion_sha: &str,
    started_utc: &str,
    finished_utc: &str,
    steps: &[StepRecord],
) -> Result<PathBuf, String> {
    fs::create_dir_all(out_dir)
        .map_err(|e| format!("create out_dir {}: {e}", out_dir.display()))?;

    let mut toml = String::new();
    toml.push_str("schema_version = 1\n");
    toml.push_str(&format!("git_sha = \"{legion_sha}\"\n"));
    toml.push_str(&format!("started_utc = \"{started_utc}\"\n"));
    toml.push_str(&format!("finished_utc = \"{finished_utc}\"\n"));
    toml.push('\n');

    let overall_status = if steps.iter().any(|s| s.status == StepStatus::Failed) {
        "failed"
    } else if steps
        .iter()
        .all(|s| s.status == StepStatus::Passed || s.status == StepStatus::Skipped)
    {
        "passed"
    } else {
        "unknown"
    };
    toml.push_str(&format!("overall_status = \"{overall_status}\"\n\n"));

    for step in steps {
        toml.push_str("[[steps]]\n");
        toml.push_str(&format!("id = \"{}\"\n", step.id));
        toml.push_str(&format!("status = \"{}\"\n", step.status.as_str()));
        toml.push_str(&format!("started_utc = \"{}\"\n", step.started_utc));
        toml.push_str(&format!("finished_utc = \"{}\"\n", step.finished_utc));
        toml.push_str(&format!("duration_ms = {}\n", step.duration_ms));
        // Truncate detail to 256 chars; no raw source excerpts.
        let detail = if step.detail.chars().count() > 256 {
            format!("{}...", step.detail.chars().take(256).collect::<String>())
        } else {
            step.detail.clone()
        };
        toml.push_str(&format!("detail = {:?}\n\n", detail));
    }

    let out_path = out_dir.join("gp1_report.toml");
    fs::write(&out_path, &toml).map_err(|e| format!("write {}: {e}", out_path.display()))?;
    eprintln!("[s7] wrote evidence: {}", out_path.display());

    if let Some(ev_dir) = evidence_dir {
        fs::create_dir_all(ev_dir)
            .map_err(|e| format!("create evidence_dir {}: {e}", ev_dir.display()))?;
        let ev_path = ev_dir.join("gp1_report.toml");
        fs::write(&ev_path, &toml)
            .map_err(|e| format!("write evidence copy {}: {e}", ev_path.display()))?;
        eprintln!("[s7] wrote evidence copy: {}", ev_path.display());
    }

    Ok(out_path)
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("golden-path-1: argument error: {e}");
            eprintln!(
                "Usage: golden_path_1 --fixture-dir <path> [--out-dir <path>] [--record-evidence <path>]"
            );
            process::exit(2);
        }
    };

    let started_utc = utc_now();
    let mut steps: Vec<StepRecord> = Vec::new();

    // Resolve Legion repo SHA (used in the evidence file; workspace root is cwd).
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let legion_sha = resolve_legion_git_sha(&cwd);
    eprintln!("[gp1] Legion git SHA: {legion_sha}");
    eprintln!("[gp1] fixture dir: {}", args.fixture_dir.display());

    macro_rules! record_step {
        ($id:expr, $status:expr, $detail:expr, $duration_ms:expr, $started:expr, $finished:expr) => {
            steps.push(StepRecord {
                id: $id,
                started_utc: $started,
                finished_utc: $finished,
                duration_ms: $duration_ms,
                status: $status,
                detail: $detail,
            });
        };
    }

    // ── s1 ──────────────────────────────────────────────────────────────────
    let s1_start = utc_now();
    let (s1_result, s1_ms) = run_timer(|| run_s1(&args.fixture_dir));
    let s1_end = utc_now();
    let (temp_dir, mut app) = match s1_result {
        Ok(r) => {
            eprintln!(
                "[s1] passed ({}ms); temp_dir={}",
                s1_ms,
                r.temp_dir.display()
            );
            record_step!(
                "s1",
                StepStatus::Passed,
                format!("fixture copied and workspace opened ({}ms)", s1_ms),
                s1_ms,
                s1_start,
                s1_end
            );
            (r.temp_dir, r.app)
        }
        Err(e) => {
            eprintln!("[s1] FAILED: {e}");
            record_step!("s1", StepStatus::Failed, e.clone(), s1_ms, s1_start, s1_end);
            // s1 failure is fatal — can't continue without workspace.
            let _ = write_evidence(
                &args.out_dir,
                args.evidence_dir.as_deref(),
                &legion_sha,
                &started_utc,
                &utc_now(),
                &steps,
            );
            process::exit(1);
        }
    };

    // ── s2 ──────────────────────────────────────────────────────────────────
    let s2_start = utc_now();
    let (s2_result, s2_ms) = run_timer(|| run_s2(&temp_dir));
    let s2_end = utc_now();
    let mut lsp_session: Option<RustAnalyzerSession> = match s2_result {
        Ok(Some(session)) => {
            eprintln!("[s2] passed — rust-analyzer session live ({}ms)", s2_ms);
            record_step!(
                "s2",
                StepStatus::Passed,
                format!("rust-analyzer session initialized ({}ms)", s2_ms),
                s2_ms,
                s2_start,
                s2_end
            );
            Some(session)
        }
        Ok(None) => {
            eprintln!("[s2] skipped — rust-analyzer not on PATH");
            record_step!(
                "s2",
                StepStatus::Skipped,
                "rust-analyzer not found on PATH".to_string(),
                s2_ms,
                s2_start,
                s2_end
            );
            None
        }
        Err(e) => {
            eprintln!("[s2] FAILED: {e}");
            record_step!("s2", StepStatus::Failed, e.clone(), s2_ms, s2_start, s2_end);
            None
        }
    };

    // ── s3 ──────────────────────────────────────────────────────────────────
    let s3_start = utc_now();
    if let Some(ref mut session) = lsp_session {
        let (s3_result, s3_ms) = run_timer(|| run_s3(&temp_dir, &mut app, session));
        let s3_end = utc_now();
        match s3_result {
            Ok(()) => {
                eprintln!("[s3] passed ({}ms)", s3_ms);
                record_step!(
                    "s3",
                    StepStatus::Passed,
                    format!("error introduced, detected, fixed, cleared ({}ms)", s3_ms),
                    s3_ms,
                    s3_start,
                    s3_end
                );
            }
            Err(e) => {
                eprintln!("[s3] FAILED: {e}");
                record_step!("s3", StepStatus::Failed, e.clone(), s3_ms, s3_start, s3_end);
            }
        }
    } else {
        let s3_end = utc_now();
        eprintln!("[s3] skipped — no LSP session (s2 skipped or failed)");
        record_step!(
            "s3",
            StepStatus::Skipped,
            "skipped: no rust-analyzer session (s2 skipped or failed)".to_string(),
            0,
            s3_start,
            s3_end
        );
    }

    // ── s4 ──────────────────────────────────────────────────────────────────
    let s4_start = utc_now();
    let (s4_result, s4_ms) = run_timer(|| run_s4(&mut app));
    let s4_end = utc_now();
    match s4_result {
        Ok(()) => {
            eprintln!("[s4] passed ({}ms)", s4_ms);
            record_step!(
                "s4",
                StepStatus::Passed,
                format!(
                    "workspace search returned hits for SMOKE_MARKER_ALPHA ({}ms)",
                    s4_ms
                ),
                s4_ms,
                s4_start,
                s4_end
            );
        }
        Err(e) => {
            eprintln!("[s4] FAILED: {e}");
            record_step!("s4", StepStatus::Failed, e.clone(), s4_ms, s4_start, s4_end);
        }
    }

    // ── s5 ──────────────────────────────────────────────────────────────────
    let s5_start = utc_now();
    let (s5_result, s5_ms) = run_timer(|| run_s5(&temp_dir, &mut app));
    let s5_end = utc_now();
    match s5_result {
        Ok(None) => {
            eprintln!("[s5] passed ({}ms)", s5_ms);
            record_step!(
                "s5",
                StepStatus::Passed,
                format!(
                    "cargo test exited 0 via product terminal gate ({}ms)",
                    s5_ms
                ),
                s5_ms,
                s5_start,
                s5_end
            );
        }
        Ok(Some(skip_reason)) => {
            eprintln!("[s5] skipped: {skip_reason}");
            record_step!(
                "s5",
                StepStatus::Skipped,
                format!("skipped: {skip_reason}"),
                s5_ms,
                s5_start,
                s5_end
            );
        }
        Err(e) => {
            eprintln!("[s5] FAILED: {e}");
            record_step!("s5", StepStatus::Failed, e.clone(), s5_ms, s5_start, s5_end);
        }
    }

    // ── s6 ──────────────────────────────────────────────────────────────────
    let s6_start = utc_now();
    let (s6_result, s6_ms) = run_timer(|| run_s6(&temp_dir, &mut app));
    let s6_end = utc_now();
    match s6_result {
        Ok(()) => {
            eprintln!("[s6] passed ({}ms)", s6_ms);
            record_step!(
                "s6",
                StepStatus::Passed,
                format!("edit-save-stage-commit cycle verified ({}ms)", s6_ms),
                s6_ms,
                s6_start,
                s6_end
            );
        }
        Err(e) => {
            eprintln!("[s6] FAILED: {e}");
            record_step!("s6", StepStatus::Failed, e.clone(), s6_ms, s6_start, s6_end);
        }
    }

    // ── s7 ──────────────────────────────────────────────────────────────────
    // Two-pass write so s7 is included in the final TOML (M-4):
    //   Pass 1: write s1-s6, capture timing.
    //   Push s7 record.
    //   Pass 2: overwrite with s1-s7 and copy to --record-evidence path.
    let s7_start = utc_now();
    let s7_wall = Instant::now();
    let finished_utc = utc_now();
    let first_result = write_evidence(
        &args.out_dir,
        None, // evidence_dir copy deferred to pass 2
        &legion_sha,
        &started_utc,
        &finished_utc,
        &steps,
    );
    let s7_ms = s7_wall.elapsed().as_millis();
    let s7_end = utc_now();

    match &first_result {
        Ok(path) => eprintln!(
            "[s7] evidence written (preliminary, s1-s6): {}",
            path.display()
        ),
        Err(e) => eprintln!("[s7] FAILED to write evidence (pass 1): {e}"),
    }
    steps.push(StepRecord {
        id: "s7",
        started_utc: s7_start,
        finished_utc: s7_end.clone(),
        duration_ms: s7_ms,
        status: if first_result.is_ok() {
            StepStatus::Passed
        } else {
            StepStatus::Failed
        },
        detail: match &first_result {
            Ok(_) => format!("evidence TOML written ({}ms)", s7_ms),
            Err(e) => e.clone(),
        },
    });

    // Pass 2: rewrite with all steps including s7, and copy to evidence_dir.
    match write_evidence(
        &args.out_dir,
        args.evidence_dir.as_deref(),
        &legion_sha,
        &started_utc,
        &s7_end,
        &steps,
    ) {
        Ok(path) => eprintln!("[s7] evidence rewritten (final, s1-s7): {}", path.display()),
        Err(e) => eprintln!("[s7] WARNING: pass-2 rewrite failed: {e}"),
    }

    // Print per-step summary.
    eprintln!("\n[gp1] SMOKE SUMMARY");
    for step in &steps {
        eprintln!(
            "  {} {} ({}ms): {}",
            step.id,
            step.status.as_str(),
            step.duration_ms,
            &step.detail[..step.detail.len().min(80)]
        );
    }

    // Clean up temp dir on success; leave it for inspection on failure.
    let any_failed = steps.iter().any(|s| s.status == StepStatus::Failed);
    if any_failed {
        eprintln!(
            "\n[gp1] FAILED — temp workspace left for inspection: {}",
            temp_dir.display()
        );
        process::exit(1);
    } else {
        eprintln!(
            "\n[gp1] PASSED — cleaning up temp workspace: {}",
            temp_dir.display()
        );
        let _ = fs::remove_dir_all(&temp_dir);
        process::exit(0);
    }
}
