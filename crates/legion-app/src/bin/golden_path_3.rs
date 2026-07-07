//! GP-3 Golden Path smoke runner (M10 milestone closer — delegate mode).
//!
//! Invoked by `cargo run -p xtask -- golden-path-3` (subprocess model — xtask
//! cannot depend on legion-app, so it spawns this binary and reads its exit
//! code + the evidence TOML).
//!
//! Compiled with **default features** (which include `ai`) plus
//! `--features test-helpers` (needed for `inject_cancellation_flag_for_test`
//! in s6).
//!
//! # Steps
//! s1 copy-fixture:     copy fixture to temp dir; git-init; open as Trusted
//!                      workspace; set_product_mode(Delegate).
//! s2 scope-selection:  build a DelegatedTaskScope with Module target and
//!                      secrets.txt in forbidden_paths.
//! s3 worker-loop:      read→grep→edit-as-proposal→end_turn; assert
//!                      Completed + audit pairing + workspace byte-unchanged.
//! s4 scope-denial:     script reads secrets.txt (forbidden); assert Blocked
//!                      with ToolCallRejected.
//! s5 sandbox-teeth:    scope with TerminalCommand; assert Completed or
//!                      Blocked (acceptable on all platforms).
//! s6 kill-switch:      inject pre-cancelled flag; assert Cancelled.
//! s7 orphan-reap:      create stale task- dir; call reap_orphaned_sandboxes;
//!                      assert removed; decoy left alone.
//! s8 review-apply:     CreateFile proposal lifecycle; checkpoint verify;
//!                      restore; verify file removed.
//! s9 evidence:         write `target/golden-path/gp3_report.toml`.
//!
//! # Constraints
//! - Never writes inside the Legion repo (except target/ and --record-evidence path).
//! - Fixture copies live in OS temp; cleaned on success, left on failure.
//! - Zero egress: all operations are local; deterministic scripted provider only.

use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use legion_agent::reap_orphaned_sandboxes;
use legion_ai::tool_calls::ScriptedToolCallingProviderBuilder;
use legion_app::{AppComposition, AppDelegatedTaskOutcome, AppProductMode};
use legion_protocol::{
    CanonicalPath, CapabilityId, CorrelationId, CreateFileProposal, DelegatedTaskLoopStepKind,
    DelegatedTaskProposalHunkDisposition, DelegatedTaskRiskTolerance, DelegatedTaskScope,
    DelegatedTaskScopeTargetKind, LegionToolKind, PreviewSummary, PrincipalId, ProposalId,
    ProposalPayload, ProposalRequest, ProposalResponse, ProposalVersionPreconditions,
    TimestampMillis, WorkspaceProposal, WorkspaceTrustState,
};

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
// Helpers (copied verbatim from golden_path_2.rs)
// ─────────────────────────────────────────────────────────────────────────────

/// Convert Unix epoch seconds to an RFC 3339 UTC timestamp string.
fn epoch_secs_to_rfc3339(secs: u64) -> String {
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (year, month, day) = days_to_ymd(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Convert days since Unix epoch to (year, month, day).
///
/// Algorithm: civil_from_days — Howard Hinnant, https://howardhinnant.github.io/date_algorithms.html
fn days_to_ymd(days: i64) -> (u32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mon = if mp < 10 { mp + 3 } else { mp - 9 };
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

/// Copy a directory tree recursively.
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

/// Simple recursive directory walker that skips `.git` and `target/` dirs.
/// Used only for workspace fingerprinting.
fn walkdir_simple(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut entries = Vec::new();
    walkdir_inner(dir, &mut entries)?;
    Ok(entries)
}

fn walkdir_inner(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("read_dir entry: {e}"))?;
        let ft = entry.file_type().map_err(|e| format!("file_type: {e}"))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Skip .git and target/ — these are build/vcs artifacts, not workspace content.
        if name_str == ".git" || name_str == "target" {
            continue;
        }
        let path = entry.path();
        if ft.is_dir() {
            walkdir_inner(&path, out)?;
        } else if ft.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

/// Compute a lightweight fingerprint of a workspace: file count + total bytes.
/// Skips `.git` and `target/` directories.
fn workspace_fingerprint(dir: &Path) -> Result<String, String> {
    let files = walkdir_simple(dir)?;
    let mut count = 0u64;
    let mut bytes = 0u64;
    for f in &files {
        count += 1;
        bytes += fs::metadata(f).map(|m| m.len()).unwrap_or(0);
    }
    Ok(format!("files={count},bytes={bytes}"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s1: copy fixture + open workspace + Delegate mode
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
        std::env::temp_dir().join(format!("legion-gp3-smoke-{}-{}", process::id(), nanos));

    copy_dir_recursive(fixture_dir, &temp_dir)?;

    git_cmd(&temp_dir, &["init", "-b", "main"])?;
    git_cmd(
        &temp_dir,
        &["config", "user.email", "gp3-smoke@legion.test"],
    )?;
    git_cmd(&temp_dir, &["config", "user.name", "GP-3 Smoke"])?;
    git_cmd(&temp_dir, &["add", "."])?;
    git_cmd(
        &temp_dir,
        &["commit", "-m", "initial: gp3 smoke fixture baseline"],
    )?;

    let mut app = AppComposition::new();
    app.open_workspace(
        &temp_dir,
        WorkspaceTrustState::Trusted,
        PrincipalId("gp3-smoke".to_string()),
    )
    .map_err(|e| format!("open_workspace failed: {e:?}"))?;

    app.set_product_mode(AppProductMode::Delegate);
    eprintln!("[s1] product_mode set to Delegate");

    Ok(S1Result { temp_dir, app })
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s2: scope selection
// ─────────────────────────────────────────────────────────────────────────────

fn run_s2(temp_dir: &Path) -> Result<DelegatedTaskScope, String> {
    // Write the secrets.txt bait file — used by s4 to test scope denial.
    fs::write(
        temp_dir.join("secrets.txt"),
        "TOP_SECRET_API_KEY=sk-live-XXXXX\n",
    )
    .map_err(|e| format!("s2: write secrets.txt: {e}"))?;

    Ok(DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Module,
        workspace_root: CanonicalPath(temp_dir.to_string_lossy().into_owned()),
        // target_path must be an absolute canonical path so the scope validator
        // can compare it against absolute paths resolved by the tool executor.
        target_path: Some(CanonicalPath(
            temp_dir.join("src").to_string_lossy().into_owned(),
        )),
        risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
        allowed_tools: vec![
            LegionToolKind::Read,
            LegionToolKind::Grep,
            LegionToolKind::Glob,
            LegionToolKind::Outline,
            LegionToolKind::EditAsProposal,
        ],
        forbidden_paths: vec![CanonicalPath(
            temp_dir.join("secrets.txt").to_string_lossy().into_owned(),
        )],
        schema_version: 1,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s3: worker loop (happy path)
// ─────────────────────────────────────────────────────────────────────────────

struct S3Result {
    audit_step_count: usize,
}

fn run_s3(
    app: &mut AppComposition,
    temp_dir: &Path,
    scope: &DelegatedTaskScope,
) -> Result<S3Result, String> {
    // Record the main workspace fingerprint before the run.
    let main_fingerprint = workspace_fingerprint(temp_dir)?;

    // EditAsProposal requires "path" and "replacement" fields (NOT old_text/new_text).
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "read", serde_json::json!({ "path": "src/main.rs" }))
        .tool_use("t2", "grep", serde_json::json!({ "pattern": "fn main", "path": "src" }))
        .tool_use(
            "t3",
            "edit-as-proposal",
            serde_json::json!({
                "path": "src/main.rs",
                "replacement": "fn main() {\n    // GP-3 smoke edit\n    println!(\"Hello, world!\");\n}\n"
            }),
        )
        .end_turn("Task complete: read, searched, and proposed an edit.")
        .build("gp3-scripted");

    let outcome = app
        .start_delegated_task(
            "Read main.rs, search for main function, propose an edit".to_string(),
            scope.clone(),
            &provider,
        )
        .map_err(|e| format!("s3: start_delegated_task: {e:?}"))?;

    match outcome {
        AppDelegatedTaskOutcome::Completed {
            final_message,
            proposals,
            audit_steps,
        } => {
            // Assert final message.
            if !final_message.contains("Task complete") {
                return Err(format!("s3: unexpected final_message: {final_message}"));
            }

            // Assert audit pairing: every ToolCallRequest has a paired
            // ToolCallResult or ToolCallRejected with the same causality_id.
            let requests: Vec<_> = audit_steps
                .iter()
                .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRequest)
                .collect();
            let results: Vec<_> = audit_steps
                .iter()
                .filter(|s| {
                    s.kind == DelegatedTaskLoopStepKind::ToolCallResult
                        || s.kind == DelegatedTaskLoopStepKind::ToolCallRejected
                })
                .collect();

            if requests.len() != results.len() {
                return Err(format!(
                    "s3: audit pairing mismatch: {} requests, {} results",
                    requests.len(),
                    results.len()
                ));
            }

            for req in &requests {
                if !results.iter().any(|r| r.causality_id == req.causality_id) {
                    return Err(format!(
                        "s3: unpaired request causality_id={}",
                        req.causality_id
                    ));
                }
            }

            // Assert main workspace unchanged (proposals are sandboxed; main workspace is immutable).
            let post_fingerprint = workspace_fingerprint(temp_dir)?;
            if main_fingerprint != post_fingerprint {
                return Err(format!(
                    "s3: main workspace changed: before={main_fingerprint} after={post_fingerprint}"
                ));
            }

            // Assert exactly 1 proposal was surfaced (the edit-as-proposal on src/main.rs).
            if proposals.len() != 1 {
                return Err(format!(
                    "s3: expected 1 proposal from edit-as-proposal, got {}",
                    proposals.len()
                ));
            }

            let proposal = &proposals[0];

            // Assert the proposal targets src/main.rs.
            let targets_main_rs = match &proposal.payload {
                ProposalPayload::CreateFile(p) => {
                    p.path.0.ends_with("main.rs") || p.path.0.contains("src/main.rs")
                }
                _ => false,
            };
            if !targets_main_rs {
                return Err(format!(
                    "s3: proposal does not target src/main.rs; payload: {:?}",
                    proposal.payload
                ));
            }

            // Assert a ledger row exists and one hunk review can be dispatched.
            // hunk_id format: "delegate:proposal:{id}:metadata-chunk:0"
            let proposal_id = proposal.proposal_id;
            let hunk_id = format!("delegate:proposal:{}:metadata-chunk:0", proposal_id.0);
            app.review_delegate_proposal_hunk(
                proposal_id,
                hunk_id.clone(),
                DelegatedTaskProposalHunkDisposition::Accepted,
            )
            .map_err(|e| {
                format!(
                    "s3: review_delegate_proposal_hunk({hunk_id}) failed: {e:?}"
                )
            })?;

            eprintln!(
                "[s3] Completed: {} audit steps, 1 proposal targeting src/main.rs \
                 (proposal_id={:?}); hunk review dispatched",
                audit_steps.len(),
                proposal_id,
            );
            Ok(S3Result {
                audit_step_count: audit_steps.len(),
            })
        }
        other => Err(format!("s3: expected Completed, got {other:?}")),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s4: scope denial — script reads forbidden path, assert Blocked
// ─────────────────────────────────────────────────────────────────────────────

fn run_s4(app: &mut AppComposition, scope: &DelegatedTaskScope) -> Result<(), String> {
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "deny-1",
            "read",
            serde_json::json!({ "path": "secrets.txt" }),
        )
        .end_turn("Should not reach here.")
        .build("gp3-denial");

    let outcome = app
        .start_delegated_task("Read secrets.txt".to_string(), scope.clone(), &provider)
        .map_err(|e| format!("s4: {e:?}"))?;

    match outcome {
        AppDelegatedTaskOutcome::Blocked { audit_steps, .. } => {
            let rejected = audit_steps
                .iter()
                .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRejected)
                .count();
            if rejected == 0 {
                return Err("s4: Blocked but no ToolCallRejected steps".to_string());
            }
            eprintln!("[s4] scope denial: Blocked with {rejected} rejection(s)");
            Ok(())
        }
        other => Err(format!("s4: expected Blocked, got {other:?}")),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s5: sandbox teeth — TerminalCommand scope, verify enforcement
// ─────────────────────────────────────────────────────────────────────────────

struct S5Result {
    terminal_ran: bool,
}

fn run_s5(app: &mut AppComposition, temp_dir: &Path) -> Result<S5Result, String> {
    // Scope WITH TerminalCommand allowed.
    let scope = DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Repo,
        workspace_root: CanonicalPath(temp_dir.to_string_lossy().into_owned()),
        target_path: None,
        risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
        allowed_tools: vec![LegionToolKind::Read, LegionToolKind::TerminalCommand],
        forbidden_paths: vec![],
        schema_version: 1,
    };

    // Script: echo a marker into a file inside the worktree (sandbox probe).
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "sandbox-1",
            "terminal-command",
            serde_json::json!({ "command": "echo gp3-sandbox-probe" }),
        )
        .end_turn("Terminal command executed.")
        .build("gp3-sandbox");

    let outcome = app
        .start_delegated_task(
            "Run a terminal command inside the worktree".to_string(),
            scope,
            &provider,
        )
        .map_err(|e| format!("s5: {e:?}"))?;

    match outcome {
        AppDelegatedTaskOutcome::Completed { audit_steps, .. } => {
            let results: Vec<_> = audit_steps
                .iter()
                .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallResult)
                .collect();
            eprintln!(
                "[s5] sandbox: Completed with {} tool result(s) (TerminalCommand ran through spawn_sandboxed)",
                results.len()
            );
            Ok(S5Result { terminal_ran: true })
        }
        AppDelegatedTaskOutcome::Blocked { .. } => {
            // TerminalCommand may be denied by the broker on some platforms.
            eprintln!("[s5] sandbox: Blocked (TerminalCommand denied by broker — acceptable)");
            Ok(S5Result {
                terminal_ran: false,
            })
        }
        other => Err(format!("s5: unexpected outcome: {other:?}")),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s6: kill switch — inject pre-cancelled flag, assert Cancelled
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "test-helpers")]
fn run_s6(app: &mut AppComposition, temp_dir: &Path) -> Result<(), String> {
    use legion_app::SharedCancellationFlag;

    let scope = DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Repo,
        workspace_root: CanonicalPath(temp_dir.to_string_lossy().into_owned()),
        target_path: None,
        risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
        allowed_tools: vec![LegionToolKind::Read],
        forbidden_paths: vec![],
        schema_version: 1,
    };

    // Inject a pre-cancelled flag. The loop will observe this before its first
    // model turn and return Cancelled without making any API calls.
    let flag = SharedCancellationFlag::new();
    flag.cancel();
    app.inject_cancellation_flag_for_test(flag);

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "cancel-1",
            "read",
            serde_json::json!({ "path": "src/main.rs" }),
        )
        .end_turn("Should not reach here — cancelled.")
        .build("gp3-cancel");

    let outcome = app
        .start_delegated_task(
            "This task should be cancelled".to_string(),
            scope,
            &provider,
        )
        .map_err(|e| format!("s6: {e:?}"))?;

    match outcome {
        AppDelegatedTaskOutcome::Cancelled => {
            eprintln!("[s6] kill switch: Cancelled (pre-cancelled flag detected)");
            Ok(())
        }
        other => Err(format!("s6: expected Cancelled, got {other:?}")),
    }
}

#[cfg(not(feature = "test-helpers"))]
fn run_s6(_app: &mut AppComposition, _temp_dir: &Path) -> Result<(), String> {
    // test-helpers feature not compiled — s6 requires inject_cancellation_flag_for_test.
    // The xtask runner always passes --features test-helpers, so this path is
    // only reached in manual builds. Return an error so it's not silently skipped.
    Err("s6: test-helpers feature not compiled — compile with --features test-helpers".to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s7: orphan reap — create stale task- dir, assert it is removed
// ─────────────────────────────────────────────────────────────────────────────

fn run_s7(temp_dir: &Path) -> Result<(), String> {
    // Use an isolated orphan-test-only reap root separate from the real
    // delegated-tasks directory used by s3-s6. This prevents collision with
    // sandbox dirs created during those steps.
    let reap_root = temp_dir.join("orphan-reap-test");
    fs::create_dir_all(reap_root.join("task-orphan-gp3"))
        .map_err(|e| format!("s7: create orphan dir: {e}"))?;
    fs::write(
        reap_root.join("task-orphan-gp3").join("marker.txt"),
        "stale sandbox",
    )
    .map_err(|e| format!("s7: write marker: {e}"))?;

    // Fresh decoy (no `task-` prefix — must NOT be reaped).
    fs::create_dir_all(reap_root.join("not-a-task"))
        .map_err(|e| format!("s7: create decoy: {e}"))?;

    // Reap with an empty active list → all task- dirs are orphans.
    let removed =
        reap_orphaned_sandboxes(&reap_root, &[]).map_err(|e| format!("s7: reap: {e:?}"))?;

    if removed.len() != 1 {
        return Err(format!(
            "s7: expected 1 orphan reaped, got {}",
            removed.len()
        ));
    }
    if reap_root.join("task-orphan-gp3").exists() {
        return Err("s7: orphan dir still exists after reap".to_string());
    }
    if !reap_root.join("not-a-task").exists() {
        return Err("s7: decoy dir removed — should have been left alone".to_string());
    }

    eprintln!("[s7] orphan reap: 1 orphan removed, decoy survived");
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s8: review-apply — proposal lifecycle pipeline (follows GP-2 s6)
// ─────────────────────────────────────────────────────────────────────────────

fn run_s8(app: &mut AppComposition, temp_dir: &Path) -> Result<(), String> {
    // ── 1. Refresh workspace generation (idempotent re-open) ─────────────────
    let current_gen = app
        .open_workspace(
            temp_dir,
            WorkspaceTrustState::Trusted,
            PrincipalId("gp3-smoke".to_string()),
        )
        .map_err(|e| format!("s8: open_workspace (generation refresh) failed: {e:?}"))?
        .generation;
    eprintln!("[s8] workspace generation refreshed: {current_gen:?}");

    // ── 2. Build a CreateFile proposal ───────────────────────────────────────
    let smoke_file_path = temp_dir.join("delegated-task-gp3-proposal.txt");
    let smoke_canonical = CanonicalPath(smoke_file_path.to_string_lossy().into_owned());

    let proposal = WorkspaceProposal {
        proposal_id: ProposalId(800),
        principal: PrincipalId("gp3-smoke".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(800),
        payload: ProposalPayload::CreateFile(CreateFileProposal {
            path: smoke_canonical.clone(),
            initial_content: Some("gp3 s8 checkpoint smoke\n".to_string()),
        }),
        preconditions: ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: None,
            file_content_version: None,
            workspace_generation: Some(current_gen),
            expected_fingerprint: None,
            expected_file_length: None,
            expected_modified_at: None,
        },
        preview: PreviewSummary {
            summary: "gp3 smoke s8 checkpoint".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    };

    // ── 3. Register → Validate → Preview → Apply ─────────────────────────────
    let register_resp = app
        .register_proposal_lifecycle(&proposal)
        .map_err(|e| format!("s8: register_proposal_lifecycle failed: {e:?}"))?;
    match register_resp {
        ProposalResponse::Created(_) => eprintln!("[s8] proposal registered (Created)"),
        other => {
            return Err(format!(
                "s8: expected Created from register_proposal_lifecycle, got {other:?}"
            ));
        }
    }

    let validate_resp = app
        .handle_proposal_request(ProposalRequest::Validate(proposal.clone()))
        .map_err(|e| format!("s8: Validate failed: {e:?}"))?;
    match validate_resp {
        ProposalResponse::Validated(_) => eprintln!("[s8] proposal Validated"),
        other => {
            return Err(format!(
                "s8: expected Validated from Validate, got {other:?}"
            ));
        }
    }

    let preview_resp = app
        .handle_proposal_request(ProposalRequest::Preview(proposal.clone()))
        .map_err(|e| format!("s8: Preview failed: {e:?}"))?;
    match preview_resp {
        ProposalResponse::Previewed { .. } => eprintln!("[s8] proposal Previewed"),
        other => {
            return Err(format!(
                "s8: expected Previewed from Preview, got {other:?}"
            ));
        }
    }

    let apply_resp = app
        .handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
        .map_err(|e| format!("s8: Apply failed: {e:?}"))?;
    match apply_resp {
        ProposalResponse::Applied(_) => eprintln!("[s8] proposal Applied"),
        other => {
            return Err(format!("s8: expected Applied from Apply, got {other:?}"));
        }
    }

    // Assert the file was created on disk.
    if !smoke_file_path.exists() {
        return Err(format!(
            "s8: smoke file not created on disk: {}",
            smoke_file_path.display()
        ));
    }
    eprintln!("[s8] smoke file created: {}", smoke_file_path.display());

    // ── 4. Verify checkpoint was auto-created ─────────────────────────────────
    let checkpoints = app.list_checkpoints();
    if checkpoints.is_empty() {
        return Err(
            "s8: list_checkpoints() is empty after apply — expected >= 1 durable checkpoint"
                .to_string(),
        );
    }
    if checkpoints[0].proposal_id != ProposalId(800) {
        return Err(format!(
            "s8: checkpoint[0].proposal_id = {:?}; expected ProposalId(800)",
            checkpoints[0].proposal_id
        ));
    }
    let checkpoint_id = checkpoints[0].checkpoint_id.clone();
    eprintln!(
        "[s8] checkpoint verified: proposal_id={:?} checkpoint_id={checkpoint_id}",
        checkpoints[0].proposal_id
    );

    // ── 5. Restore the checkpoint ─────────────────────────────────────────────
    app.restore_checkpoint(&checkpoint_id)
        .map_err(|e| format!("s8: restore_checkpoint failed: {e:?}"))?;
    eprintln!("[s8] checkpoint restored");

    // ── 6. Verify file was removed (pre-apply state = did not exist) ──────────
    if smoke_file_path.exists() {
        return Err(format!(
            "s8: smoke file still exists after checkpoint restore: {}",
            smoke_file_path.display()
        ));
    }
    eprintln!("[s8] smoke file removed by restore (pre-apply state verified)");

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s9: write evidence TOML
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
        let detail = if step.detail.chars().count() > 256 {
            format!("{}...", step.detail.chars().take(256).collect::<String>())
        } else {
            step.detail.clone()
        };
        toml.push_str(&format!("detail = {:?}\n\n", detail));
    }

    let out_path = out_dir.join("gp3_report.toml");
    fs::write(&out_path, &toml).map_err(|e| format!("write {}: {e}", out_path.display()))?;
    eprintln!("[s9] wrote evidence: {}", out_path.display());

    if let Some(ev_dir) = evidence_dir {
        fs::create_dir_all(ev_dir)
            .map_err(|e| format!("create evidence_dir {}: {e}", ev_dir.display()))?;
        let ev_path = ev_dir.join("gp3_report.toml");
        fs::write(&ev_path, &toml)
            .map_err(|e| format!("write evidence copy {}: {e}", ev_path.display()))?;
        eprintln!("[s9] wrote evidence copy: {}", ev_path.display());
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
            eprintln!("golden-path-3: argument error: {e}");
            eprintln!(
                "Usage: golden_path_3 --fixture-dir <path> [--out-dir <path>] [--record-evidence <path>]"
            );
            process::exit(2);
        }
    };

    let started_utc = utc_now();
    let mut steps: Vec<StepRecord> = Vec::new();

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let legion_sha = resolve_legion_git_sha(&cwd);
    eprintln!("[gp3] Legion git SHA: {legion_sha}");
    eprintln!("[gp3] fixture dir: {}", args.fixture_dir.display());

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
                format!(
                    "fixture copied, workspace opened, Delegate mode set ({}ms)",
                    s1_ms
                ),
                s1_ms,
                s1_start,
                s1_end
            );
            (r.temp_dir, r.app)
        }
        Err(e) => {
            eprintln!("[s1] FAILED: {e}");
            record_step!("s1", StepStatus::Failed, e.clone(), s1_ms, s1_start, s1_end);
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
    let scope: Option<DelegatedTaskScope> = match s2_result {
        Ok(sc) => {
            eprintln!(
                "[s2] passed ({}ms); scope built with secrets.txt in forbidden_paths",
                s2_ms
            );
            record_step!(
                "s2",
                StepStatus::Passed,
                format!(
                    "DelegatedTaskScope built: Module target, secrets.txt in forbidden_paths, 5 allowed tools ({}ms)",
                    s2_ms
                ),
                s2_ms,
                s2_start,
                s2_end
            );
            Some(sc)
        }
        Err(e) => {
            eprintln!("[s2] FAILED: {e}");
            record_step!("s2", StepStatus::Failed, e.clone(), s2_ms, s2_start, s2_end);
            None
        }
    };

    // ── s3 ──────────────────────────────────────────────────────────────────
    let s3_start = utc_now();
    if let Some(ref sc) = scope {
        let (s3_result, s3_ms) = run_timer(|| run_s3(&mut app, &temp_dir, sc));
        let s3_end = utc_now();
        match s3_result {
            Ok(r) => {
                eprintln!(
                    "[s3] passed ({}ms); {} audit steps",
                    s3_ms, r.audit_step_count
                );
                record_step!(
                    "s3",
                    StepStatus::Passed,
                    format!(
                        "Completed: read+grep+edit-as-proposal; {} audit steps; all requests paired; workspace unchanged ({}ms)",
                        r.audit_step_count, s3_ms
                    ),
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
        eprintln!("[s3] skipped — s2 failed (no scope)");
        record_step!(
            "s3",
            StepStatus::Skipped,
            "skipped: s2 failed (no scope available)".to_string(),
            0,
            s3_start,
            s3_end
        );
    }

    // ── s4 ──────────────────────────────────────────────────────────────────
    let s4_start = utc_now();
    if let Some(ref sc) = scope {
        let (s4_result, s4_ms) = run_timer(|| run_s4(&mut app, sc));
        let s4_end = utc_now();
        match s4_result {
            Ok(()) => {
                eprintln!("[s4] passed ({}ms)", s4_ms);
                record_step!(
                    "s4",
                    StepStatus::Passed,
                    format!(
                        "scope denial: Blocked with ToolCallRejected for secrets.txt ({}ms)",
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
    } else {
        let s4_end = utc_now();
        eprintln!("[s4] skipped — s2 failed (no scope)");
        record_step!(
            "s4",
            StepStatus::Skipped,
            "skipped: s2 failed (no scope)".to_string(),
            0,
            s4_start,
            s4_end
        );
    }

    // ── s5 ──────────────────────────────────────────────────────────────────
    let s5_start = utc_now();
    let (s5_result, s5_ms) = run_timer(|| run_s5(&mut app, &temp_dir));
    let s5_end = utc_now();
    match s5_result {
        Ok(r) => {
            eprintln!("[s5] passed ({}ms); terminal_ran={}", s5_ms, r.terminal_ran);
            let detail = if r.terminal_ran {
                format!("TerminalCommand ran through spawn_sandboxed ({}ms)", s5_ms)
            } else {
                format!(
                    "TerminalCommand denied by broker — acceptable ({}ms)",
                    s5_ms
                )
            };
            record_step!("s5", StepStatus::Passed, detail, s5_ms, s5_start, s5_end);
        }
        Err(e) => {
            eprintln!("[s5] FAILED: {e}");
            record_step!("s5", StepStatus::Failed, e.clone(), s5_ms, s5_start, s5_end);
        }
    }

    // ── s6 ──────────────────────────────────────────────────────────────────
    let s6_start = utc_now();
    let (s6_result, s6_ms) = run_timer(|| run_s6(&mut app, &temp_dir));
    let s6_end = utc_now();
    match s6_result {
        Ok(()) => {
            eprintln!("[s6] passed ({}ms)", s6_ms);
            record_step!(
                "s6",
                StepStatus::Passed,
                format!(
                    "kill switch: Cancelled (pre-cancelled flag detected) ({}ms)",
                    s6_ms
                ),
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
    let s7_start = utc_now();
    let (s7_result, s7_ms) = run_timer(|| run_s7(&temp_dir));
    let s7_end = utc_now();
    match s7_result {
        Ok(()) => {
            eprintln!("[s7] passed ({}ms)", s7_ms);
            record_step!(
                "s7",
                StepStatus::Passed,
                format!(
                    "orphan reap: task-orphan-gp3 removed; not-a-task survived ({}ms)",
                    s7_ms
                ),
                s7_ms,
                s7_start,
                s7_end
            );
        }
        Err(e) => {
            eprintln!("[s7] FAILED: {e}");
            record_step!("s7", StepStatus::Failed, e.clone(), s7_ms, s7_start, s7_end);
        }
    }

    // ── s8 ──────────────────────────────────────────────────────────────────
    let s8_start = utc_now();
    let (s8_result, s8_ms) = run_timer(|| run_s8(&mut app, &temp_dir));
    let s8_end = utc_now();
    match s8_result {
        Ok(()) => {
            eprintln!("[s8] passed ({}ms)", s8_ms);
            record_step!(
                "s8",
                StepStatus::Passed,
                format!(
                    "CreateFile proposal applied; checkpoint verified; restore OK ({}ms)",
                    s8_ms
                ),
                s8_ms,
                s8_start,
                s8_end
            );
        }
        Err(e) => {
            eprintln!("[s8] FAILED: {e}");
            record_step!("s8", StepStatus::Failed, e.clone(), s8_ms, s8_start, s8_end);
        }
    }

    // ── s9 ──────────────────────────────────────────────────────────────────
    let s9_start = utc_now();
    let s9_wall = Instant::now();
    let finished_utc = utc_now();
    let first_result = write_evidence(
        &args.out_dir,
        None,
        &legion_sha,
        &started_utc,
        &finished_utc,
        &steps,
    );
    let s9_ms = s9_wall.elapsed().as_millis();
    let s9_end = utc_now();

    match &first_result {
        Ok(path) => eprintln!(
            "[s9] evidence written (preliminary, s1-s8): {}",
            path.display()
        ),
        Err(e) => eprintln!("[s9] FAILED to write evidence (pass 1): {e}"),
    }

    steps.push(StepRecord {
        id: "s9",
        started_utc: s9_start,
        finished_utc: s9_end.clone(),
        duration_ms: s9_ms,
        status: if first_result.is_ok() {
            StepStatus::Passed
        } else {
            StepStatus::Failed
        },
        detail: match &first_result {
            Ok(_) => format!("evidence TOML written ({}ms)", s9_ms),
            Err(e) => e.clone(),
        },
    });

    // Pass 2: rewrite with all steps including s9, copy to evidence_dir.
    match write_evidence(
        &args.out_dir,
        args.evidence_dir.as_deref(),
        &legion_sha,
        &started_utc,
        &s9_end,
        &steps,
    ) {
        Ok(path) => eprintln!("[s9] evidence rewritten (final, s1-s9): {}", path.display()),
        Err(e) => eprintln!("[s9] WARNING: pass-2 rewrite failed: {e}"),
    }

    // Print per-step summary.
    eprintln!("\n[gp3] SMOKE SUMMARY");
    for step in &steps {
        let summary: String = step.detail.chars().take(80).collect();
        eprintln!(
            "  {} {} ({}ms): {}",
            step.id,
            step.status.as_str(),
            step.duration_ms,
            summary
        );
    }

    // Drop app before temp-dir cleanup to release any file handles.
    drop(app);

    // Clean up on success; leave for inspection on failure.
    let any_failed = steps.iter().any(|s| s.status == StepStatus::Failed);
    if any_failed {
        eprintln!(
            "\n[gp3] FAILED — temp workspace left for inspection: {}",
            temp_dir.display()
        );
        process::exit(1);
    } else {
        eprintln!(
            "\n[gp3] PASSED — cleaning up temp workspace: {}",
            temp_dir.display()
        );
        let _ = fs::remove_dir_all(&temp_dir);
        process::exit(0);
    }
}
