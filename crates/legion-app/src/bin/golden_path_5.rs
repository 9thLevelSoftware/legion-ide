//! GP-5 Golden Path acceptance smoke runner.
//!
//! Invoked by `cargo run -p xtask -- golden-path-5` (subprocess model -- xtask
//! cannot depend on legion-app, so it spawns this binary and reads its exit
//! code + the evidence TOML).
//!
//! # Steps
//! s1 copy-fixture:  copy fixture to temp dir; git-init; open as Trusted workspace.
//! s2 open-file:     open main.rs from fixture, assert buffer loaded.
//! s3 edit-and-save: call edit_active_buffer + save_active_buffer; verify file changed on disk.
//! s4 syntax-check:  TreeSitterParser highlight_captures_from_text; assert non-empty captures.
//! s5 terminal-echo: launch terminal, run `echo gp5-ok`, verify output (SKIP if no PTY).
//! s6 git-commit:    refresh git; stage hunk; commit changes; verify clean status.
//! s7 evidence:      write `target/golden-path/gp5_report.toml`.
//!
//! # Constraints
//! - Never writes inside the Legion repo (except target/ and --record-evidence path).
//! - Fixture copies live in OS temp; cleaned on success, left on failure.
//! - Zero egress: all operations are local.

use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_editor::{TextEdit, TextPosition, TextRange};
use legion_index::TreeSitterParser;
use legion_protocol::{
    LanguageId, PrincipalId, TerminalPanelStatusKind, TerminalSessionId, WorkspaceTrustState,
};
use legion_ui::{CommandDispatchIntent, GitHunkStageProjection};

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

// ─────────────────────────────────────────────────────────────────────────────
// Terminal polling
// ─────────────────────────────────────────────────────────────────────────────

const TERMINAL_POLL_DEADLINE_SECS: u64 = 60;

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
        let row_count = projection.output_rows.len();
        if row_count != last_row_count || projection.status.kind != last_status {
            eprintln!(
                "[s5-poll] rows={} status={:?}",
                row_count, projection.status.kind
            );
            last_row_count = row_count;
            last_status = projection.status.kind;
        }
        for row in &projection.output_rows {
            if row.redacted_payload.contains(marker) {
                return Some(row.redacted_payload.clone());
            }
        }
        let session_done = matches!(
            projection.status.kind,
            TerminalPanelStatusKind::Exited
                | TerminalPanelStatusKind::Failed
                | TerminalPanelStatusKind::Crashed
        );
        if session_done || Instant::now() >= deadline {
            eprintln!("[s5-poll] loop exit: session_done={session_done} rows={row_count}");
            for (i, row) in projection.output_rows.iter().enumerate() {
                eprintln!(
                    "[s5-poll] row[{i}] len={} payload={:?}",
                    row.redacted_payload.len(),
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
        std::env::temp_dir().join(format!("legion-gp5-smoke-{}-{}", process::id(), nanos));

    copy_dir_recursive(fixture_dir, &temp_dir)?;

    // git init in the temp dir
    git_cmd(&temp_dir, &["init", "-b", "main"])?;
    git_cmd(
        &temp_dir,
        &["config", "user.email", "gp5-smoke@legion.test"],
    )?;
    git_cmd(&temp_dir, &["config", "user.name", "GP-5 Smoke"])?;
    git_cmd(&temp_dir, &["add", "."])?;
    git_cmd(
        &temp_dir,
        &["commit", "-m", "initial: smoke fixture baseline"],
    )?;

    let mut app = AppComposition::new();
    app.open_workspace(
        &temp_dir,
        WorkspaceTrustState::Trusted,
        PrincipalId("gp5-smoke".to_string()),
    )
    .map_err(|e| format!("open_workspace failed: {e:?}"))?;

    Ok(S1Result { temp_dir, app })
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s2: open file
// ─────────────────────────────────────────────────────────────────────────────

fn run_s2(temp_dir: &Path, app: &mut AppComposition) -> Result<(), String> {
    let main_rs = temp_dir.join("src").join("main.rs");
    let main_rs_str = main_rs.to_string_lossy().into_owned();

    app.open_file(&main_rs_str)
        .map_err(|e| format!("open main.rs: {e:?}"))?;

    let buffer_id = app
        .active_buffer_id()
        .ok_or("s2: no active buffer after open_file(main.rs)")?;

    // Assert buffer text is non-empty.
    let text = app
        .editor()
        .text(buffer_id)
        .map_err(|e| format!("s2: read buffer text: {e:?}"))?
        .to_string();
    if text.is_empty() {
        return Err("s2: buffer loaded but text is empty".to_string());
    }

    eprintln!(
        "[s2] buffer loaded: {} bytes, buffer_id={:?}",
        text.len(),
        buffer_id
    );
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s3: edit and save
// ─────────────────────────────────────────────────────────────────────────────

fn run_s3(temp_dir: &Path, app: &mut AppComposition) -> Result<(), String> {
    let main_rs = temp_dir.join("src").join("main.rs");

    let buffer_id = app
        .active_buffer_id()
        .ok_or("s3: no active buffer")?;

    let text = app
        .editor()
        .text(buffer_id)
        .map_err(|e| format!("s3: read buffer text: {e:?}"))?
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
        "\n// smoke-edited-by-gp5\n",
    );
    app.edit_active_buffer(append_edit)
        .map_err(|e| format!("s3: edit_active_buffer: {e:?}"))?;
    app.save_active_buffer()
        .map_err(|e| format!("s3: save_active_buffer: {e:?}"))?;

    // Verify file changed on disk.
    let disk_content =
        fs::read_to_string(&main_rs).map_err(|e| format!("s3: read main.rs from disk: {e}"))?;
    if !disk_content.contains("smoke-edited-by-gp5") {
        return Err(
            "s3: saved file does not contain the expected edit marker on disk".to_string(),
        );
    }

    eprintln!("[s3] edit+save verified on disk ({} bytes)", disk_content.len());
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s4: syntax check
// ─────────────────────────────────────────────────────────────────────────────

fn run_s4(temp_dir: &Path) -> Result<(), String> {
    let main_rs = temp_dir.join("src").join("main.rs");
    let text =
        fs::read_to_string(&main_rs).map_err(|e| format!("s4: read main.rs: {e}"))?;

    let parser = TreeSitterParser::new();
    let lang_id = LanguageId("rust".to_string());
    let captures = parser
        .highlight_captures_from_text(&lang_id, &text)
        .map_err(|e| format!("s4: highlight_captures_from_text failed: {e:?}"))?;

    if captures.is_empty() {
        return Err(
            "s4: expected non-empty highlight captures for Rust source; got 0".to_string(),
        );
    }

    eprintln!("[s4] syntax check passed: {} highlight captures", captures.len());
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s5: terminal echo
// ─────────────────────────────────────────────────────────────────────────────

const GP5_MARKER: &str = "GP5_ECHO_OK";

fn run_s5(app: &mut AppComposition) -> Result<Option<String>, String> {
    eprintln!("[s5] launching terminal (trusted workspace; product gate) ...");
    let launch_outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "gp5-smoke-echo".to_string(),
            timeout_secs: Some(60),
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
        eprintln!("[s5] terminal not running -- SKIP: {reason}");
        return Ok(Some(reason));
    }

    let session_id = launch_projection
        .active_session_id
        .ok_or("s5: terminal running but no active session id")?;

    // Build the echo command. On Windows, cmd.exe is the terminal shell;
    // on Unix, it's sh/bash.
    let terminal_cmd = if cfg!(windows) {
        format!("echo {GP5_MARKER}\r\n")
    } else {
        format!("echo {GP5_MARKER}\n")
    };

    eprintln!("[s5] sending command: {}", terminal_cmd.trim());
    let _ = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalInput {
            session_id,
            payload: terminal_cmd,
        })
        .map_err(|e| format!("terminal input: {e:?}"))?;

    // Poll until the echo marker appears or deadline elapses.
    let deadline = Instant::now() + Duration::from_secs(TERMINAL_POLL_DEADLINE_SECS);
    eprintln!(
        "[s5] polling for '{GP5_MARKER}' (up to {}s) ...",
        TERMINAL_POLL_DEADLINE_SECS
    );
    let hit = poll_terminal_for_marker(app, session_id, GP5_MARKER, deadline);

    match hit {
        None => Err(format!(
            "s5: timeout ({TERMINAL_POLL_DEADLINE_SECS}s) waiting for '{GP5_MARKER}' in terminal output"
        )),
        Some(row) => {
            eprintln!("[s5] echo marker found in row: {row:?}");
            Ok(None) // not skipped
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s6: git commit
// ─────────────────────────────────────────────────────────────────────────────

fn run_s6(temp_dir: &Path, app: &mut AppComposition) -> Result<(), String> {
    eprintln!("[s6] refreshing git projection ...");

    // RefreshGit -- expect dirty file from s3's edit.
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
            message: "smoke: gp5 git workflow verification".to_string(),
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

    if !committed.changed_files.is_empty() {
        return Err(format!(
            "s6: worktree not clean after commit; {} changed file(s) still present",
            committed.changed_files.len()
        ));
    }

    // Verify git log shows our commit.
    let log = git_cmd(temp_dir, &["log", "-1", "--pretty=%s"])
        .map_err(|e| format!("s6: git log: {e}"))?;
    if !log.trim().contains("smoke: gp5 git workflow verification") {
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
        let detail = if step.detail.chars().count() > 256 {
            format!("{}...", step.detail.chars().take(256).collect::<String>())
        } else {
            step.detail.clone()
        };
        toml.push_str(&format!("detail = {:?}\n\n", detail));
    }

    let out_path = out_dir.join("gp5_report.toml");
    fs::write(&out_path, &toml).map_err(|e| format!("write {}: {e}", out_path.display()))?;
    eprintln!("[s7] wrote evidence: {}", out_path.display());

    if let Some(ev_dir) = evidence_dir {
        fs::create_dir_all(ev_dir)
            .map_err(|e| format!("create evidence_dir {}: {e}", ev_dir.display()))?;
        let ev_path = ev_dir.join("gp5_report.toml");
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
            eprintln!("golden-path-5: argument error: {e}");
            eprintln!(
                "Usage: golden_path_5 --fixture-dir <path> [--out-dir <path>] [--record-evidence <path>]"
            );
            process::exit(2);
        }
    };

    let started_utc = utc_now();
    let mut steps: Vec<StepRecord> = Vec::new();

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let legion_sha = resolve_legion_git_sha(&cwd);
    eprintln!("[gp5] Legion git SHA: {legion_sha}");
    eprintln!("[gp5] fixture dir: {}", args.fixture_dir.display());

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
    let (s2_result, s2_ms) = run_timer(|| run_s2(&temp_dir, &mut app));
    let s2_end = utc_now();
    match s2_result {
        Ok(()) => {
            eprintln!("[s2] passed ({}ms)", s2_ms);
            record_step!(
                "s2",
                StepStatus::Passed,
                format!("main.rs opened and buffer loaded ({}ms)", s2_ms),
                s2_ms,
                s2_start,
                s2_end
            );
        }
        Err(e) => {
            eprintln!("[s2] FAILED: {e}");
            record_step!("s2", StepStatus::Failed, e.clone(), s2_ms, s2_start, s2_end);
        }
    }

    // ── s3 ──────────────────────────────────────────────────────────────────
    let s3_start = utc_now();
    let (s3_result, s3_ms) = run_timer(|| run_s3(&temp_dir, &mut app));
    let s3_end = utc_now();
    match s3_result {
        Ok(()) => {
            eprintln!("[s3] passed ({}ms)", s3_ms);
            record_step!(
                "s3",
                StepStatus::Passed,
                format!("edit+save verified on disk ({}ms)", s3_ms),
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

    // ── s4 ──────────────────────────────────────────────────────────────────
    let s4_start = utc_now();
    let (s4_result, s4_ms) = run_timer(|| run_s4(&temp_dir));
    let s4_end = utc_now();
    match s4_result {
        Ok(()) => {
            eprintln!("[s4] passed ({}ms)", s4_ms);
            record_step!(
                "s4",
                StepStatus::Passed,
                format!("TreeSitterParser highlight captures non-empty ({}ms)", s4_ms),
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
    let (s5_result, s5_ms) = run_timer(|| run_s5(&mut app));
    let s5_end = utc_now();
    match s5_result {
        Ok(None) => {
            eprintln!("[s5] passed ({}ms)", s5_ms);
            record_step!(
                "s5",
                StepStatus::Passed,
                format!("echo marker received via product terminal gate ({}ms)", s5_ms),
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
                format!("stage-commit cycle verified ({}ms)", s6_ms),
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
    let s7_wall = Instant::now();
    let finished_utc = utc_now();
    let first_result = write_evidence(
        &args.out_dir,
        None,
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
    eprintln!("\n[gp5] SMOKE SUMMARY");
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
            "\n[gp5] FAILED -- temp workspace left for inspection: {}",
            temp_dir.display()
        );
        process::exit(1);
    } else {
        eprintln!(
            "\n[gp5] PASSED -- cleaning up temp workspace: {}",
            temp_dir.display()
        );
        let _ = fs::remove_dir_all(&temp_dir);
        process::exit(0);
    }
}
