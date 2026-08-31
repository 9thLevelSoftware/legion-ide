//! Real product performance workloads for the perf harness (P8.F4.T1).
//!
//! `xtask` may not depend on `legion-app`/`legion-editor` (enforced by
//! `cargo run -p xtask -- check-deps`), so every workload that needs the real
//! editor, the real app composition, or the real workspace search has to live
//! on this side of the boundary. This binary runs them, writes a TOML report,
//! and `xtask` reads that report back and owns the budgets — the same
//! subprocess model `large_file_perf` and `legion-desktop --manual-perf`
//! already use.
//!
//! What each workload measures, and why:
//!
//! * **startup** — `AppComposition::new()` → `open_workspace(Legion repo)` →
//!   `open_file` → first viewport projection. That chain is what "open to
//!   ready" means for a user: the window is useless until a projection exists.
//! * **input_to_paint** — a real keystroke through `edit_active_buffer`
//!   followed by the real `viewport_projection` that a frame would paint from.
//!   Everything a keypress costs before pixels, minus the GPU submit that a
//!   headless process cannot perform (the renderer half is measured separately
//!   by `legion-desktop --manual-perf`).
//! * **scroll_jank** — real viewport projections at scroll offsets spread over
//!   the whole document. A projection that is fast only near the top has not
//!   solved scrolling.
//! * **memory_ceiling** — the real snapshot footprint of a real 1.5MB source
//!   file opened through the real product open path, not a generated string.
//! * **legion_repo** — the real product workspace search (`RunSearch`) over
//!   this repository. The reference workspace named in the master plan is this
//!   repo, and it is on disk whenever this runs.
//! * **fixture_100k_files** — 100K generated files opened as a real workspace
//!   and searched through the same product path. Generation time is excluded
//!   from the measured region and reported separately: the harness gates on
//!   search, not on how fast the OS can create files.
//!
//! The 100MB single-file workload lives in `large_file_perf` and stays there.

use std::{
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_editor::{TextEdit, TextPosition, TextRange};
use legion_protocol::{
    EditorViewportRequest, PrincipalId, ViewportDimensions, ViewportScroll, WorkspaceTrustState,
};
use legion_ui::{CommandDispatchIntent, SearchScopeProjection};

/// The reference document for the editor workloads: the largest real source
/// file in the workspace. A real file rather than a generated one, because
/// generated text has uniform line lengths and the line index is exactly what
/// these workloads stress.
const REFERENCE_DOCUMENT: &str = "crates/legion-app/src/lib.rs";

/// Keystrokes sampled for the input-to-paint percentiles.
const INPUT_SAMPLES: usize = 64;

const GIT_DISPATCH_SAMPLES: usize = 64;

/// Viewport projections sampled for the scroll percentiles.
const SCROLL_SAMPLES: usize = 64;

/// Files generated for the 100K-file reference workspace.
const FIXTURE_FILE_COUNT: usize = 100_000;

/// Files per directory in that fixture. One flat directory of 100K entries is
/// pathological on several filesystems, and the product never sees one.
const FIXTURE_FILES_PER_DIR: usize = 1_000;

/// A literal that occurs exactly once in this repository — in this line. The
/// repo search workload needs a needle rare enough that the walker cannot stop
/// early on the result limit, and present enough to prove file contents were
/// actually read rather than merely listed.
const REPO_NEEDLE: &str = "LEGION_PERF_REPO_NEEDLE_MARKER";

/// Needle planted in exactly one file of the generated 100K fixture, for the
/// same reason.
const FIXTURE_NEEDLE: &str = "LEGION_PERF_FIXTURE_NEEDLE_MARKER";

/// A viewport the size of a maximized editor pane on a 1080p display. Both
/// scroll and input-to-paint project through this, so it must be a realistic
/// row count: a 3-row viewport would make any projection look fast.
const VIEWPORT_WIDTH_PX: u32 = 1_600;
const VIEWPORT_HEIGHT_PX: u32 = 1_024;

/// One workload's measured result.
///
/// `measured` is the field that keeps this report honest. A workload that
/// could not run reports `measured = false` with the reason, and the harness
/// treats that as a failure rather than a quiet skip — the whole point of
/// P8.F4.T2 is that no OS may silently drop a workload.
struct WorkloadRecord {
    name: &'static str,
    measured: bool,
    sample_count: usize,
    fixture_bytes: u64,
    p50_micros: u64,
    p95_micros: u64,
    total_micros: u64,
    /// Byte-valued result for workloads whose metric is memory rather than
    /// time. Zero for time-valued workloads.
    bytes_value: u64,
    detail: String,
}

impl WorkloadRecord {
    fn unmeasured(name: &'static str, reason: impl Into<String>) -> Self {
        Self {
            name,
            measured: false,
            sample_count: 0,
            fixture_bytes: 0,
            p50_micros: 0,
            p95_micros: 0,
            total_micros: 0,
            bytes_value: 0,
            detail: reason.into(),
        }
    }
}

struct Args {
    report_path: PathBuf,
    workspace_root: PathBuf,
    /// Skip the 100K-file fixture. Exists only so the harness's own tests can
    /// exercise the report path in seconds; the harness never passes it, so no
    /// product workload is behind a flag in any real run.
    skip_fixture_100k: bool,
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(err) => {
            eprintln!("product_perf: {err}");
            std::process::exit(2);
        }
    };

    let mut records = Vec::new();

    // Startup has to be first and has to own its own AppComposition: a second
    // open in the same process measures warm caches, not startup.
    let (startup_record, ready) = measure_startup(&args.workspace_root);
    records.push(startup_record);

    match ready {
        Some(mut ready) => {
            records.push(measure_input_to_paint(&mut ready));
            records.push(measure_scroll(&mut ready));
            records.push(measure_memory_ceiling(&ready));
            records.push(measure_legion_repo_search(&mut ready));
            records.extend(measure_git_dispatch(&mut ready));
        }
        None => {
            for name in [
                "p8.input_to_paint",
                "p8.scroll_jank",
                "p8.memory_ceiling",
                "p8.legion_repo",
                "git.ui_dispatch_refresh",
                "git.remote_push_does_not_block_dispatch",
            ] {
                records.push(WorkloadRecord::unmeasured(
                    name,
                    "workspace did not reach ready; see p8.startup detail",
                ));
            }
        }
    }

    if args.skip_fixture_100k {
        records.push(WorkloadRecord::unmeasured(
            "p8.fixture_100k_files",
            "--skip-fixture-100k was passed; this flag exists for harness self-tests only",
        ));
    } else {
        records.push(measure_fixture_100k());
    }

    let report = render_report(&args.workspace_root, &records);
    if let Some(parent) = args.report_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::File::create(&args.report_path)
        .and_then(|mut file| file.write_all(report.as_bytes()))
    {
        Ok(()) => {
            for record in &records {
                println!(
                    "product_perf: {} measured={} p50_us={} p95_us={} bytes={} {}",
                    record.name,
                    record.measured,
                    record.p50_micros,
                    record.p95_micros,
                    record.bytes_value,
                    record.detail
                );
            }
            println!("product_perf: wrote {}", args.report_path.display());
        }
        Err(err) => {
            eprintln!("product_perf: cannot write report: {err}");
            std::process::exit(1);
        }
    }

    // Exit non-zero when a workload could not be measured so a caller that
    // ignores the report still learns something went wrong.
    if records.iter().any(|record| !record.measured) {
        std::process::exit(3);
    }
}

fn measure_git_dispatch(ready: &mut ReadyWorkspace) -> Vec<WorkloadRecord> {
    let mut refresh_samples = Vec::with_capacity(GIT_DISPATCH_SAMPLES);
    for _ in 0..GIT_DISPATCH_SAMPLES {
        let start = Instant::now();
        let outcome = ready
            .app
            .dispatch_ui_intent(CommandDispatchIntent::RefreshGit);
        let elapsed = start.elapsed();
        if let Err(err) = outcome {
            return vec![
                WorkloadRecord::unmeasured(
                    "git.ui_dispatch_refresh",
                    format!("RefreshGit dispatch failed: {err:?}"),
                ),
                WorkloadRecord::unmeasured(
                    "git.remote_push_does_not_block_dispatch",
                    "RefreshGit dispatch failed before remote measurement",
                ),
            ];
        }
        refresh_samples.push(elapsed);
    }

    let mut remote_samples = Vec::with_capacity(GIT_DISPATCH_SAMPLES);
    for _ in 0..GIT_DISPATCH_SAMPLES {
        let start = Instant::now();
        let outcome = ready
            .app
            .dispatch_ui_intent(CommandDispatchIntent::PushGitRemote {
                remote: "perf-denied-remote".to_string(),
            });
        let elapsed = start.elapsed();
        if let Err(err) = outcome {
            return vec![
                finish_duration_workload(
                    "git.ui_dispatch_refresh",
                    refresh_samples,
                    0,
                    "RefreshGit intent-to-return samples; no next paint or worker completion included".to_string(),
                ),
                WorkloadRecord::unmeasured(
                    "git.remote_push_does_not_block_dispatch",
                    format!("policy-denied PushGitRemote dispatch failed: {err:?}"),
                ),
            ];
        }
        remote_samples.push(elapsed);
    }

    vec![
        finish_duration_workload(
            "git.ui_dispatch_refresh",
            refresh_samples,
            0,
            "RefreshGit intent-to-return samples on the cheap projection path; no next paint or worker completion included".to_string(),
        ),
        finish_duration_workload(
            "git.remote_push_does_not_block_dispatch",
            remote_samples,
            0,
            "policy-denied PushGitRemote intent-to-return samples; no remote process or network allowed".to_string(),
        ),
    ]
}

fn parse_args() -> Result<Args, String> {
    let mut report_path: Option<PathBuf> = None;
    let mut workspace_root: Option<PathBuf> = None;
    let mut skip_fixture_100k = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--report" => report_path = args.next().map(PathBuf::from),
            "--workspace-root" => workspace_root = args.next().map(PathBuf::from),
            "--skip-fixture-100k" => skip_fixture_100k = true,
            _ => {}
        }
    }
    Ok(Args {
        report_path: report_path.ok_or("--report <path> is required")?,
        workspace_root: workspace_root
            .or_else(|| std::env::current_dir().ok())
            .ok_or("--workspace-root <path> is required")?,
        skip_fixture_100k,
    })
}

/// A workspace that reached "ready": open, with a document projected.
struct ReadyWorkspace {
    app: AppComposition,
    document_bytes: u64,
    line_count: usize,
    /// Snapshot footprint the moment the document became ready, before any
    /// edit. Captured here because the ceiling workload runs after the typing
    /// workload — the number that matters is the one after a user has been
    /// typing, and the at-open number is what says whether a rise came from
    /// opening or from editing.
    open_footprint_bytes: u64,
}

fn measure_startup(workspace_root: &Path) -> (WorkloadRecord, Option<ReadyWorkspace>) {
    let document = workspace_root.join(REFERENCE_DOCUMENT);
    let document_bytes = std::fs::metadata(&document)
        .map(|meta| meta.len())
        .unwrap_or(0);
    if document_bytes == 0 {
        return (
            WorkloadRecord::unmeasured(
                "p8.startup",
                format!(
                    "reference document `{}` is missing or empty",
                    document.display()
                ),
            ),
            None,
        );
    }

    let start = Instant::now();
    let mut app = AppComposition::new();
    let compose_elapsed = start.elapsed();
    if let Err(err) = app.open_workspace(
        workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId("perf-harness".to_string()),
    ) {
        return (
            WorkloadRecord::unmeasured("p8.startup", format!("open_workspace failed: {err:?}")),
            None,
        );
    }
    let workspace_elapsed = start.elapsed();
    if let Err(err) = app.open_file(document.to_string_lossy().as_ref()) {
        return (
            WorkloadRecord::unmeasured("p8.startup", format!("open_file failed: {err:?}")),
            None,
        );
    }
    let open_file_elapsed = start.elapsed();
    let Some(buffer_id) = app.active_buffer_id() else {
        return (
            WorkloadRecord::unmeasured("p8.startup", "no active buffer after open_file"),
            None,
        );
    };
    let projection = match app
        .editor()
        .viewport_projection(viewport_request(buffer_id, 0))
    {
        Ok(projection) => projection,
        Err(err) => {
            return (
                WorkloadRecord::unmeasured(
                    "p8.startup",
                    format!("first viewport projection failed: {err:?}"),
                ),
                None,
            );
        }
    };
    let elapsed = start.elapsed();

    let (line_count, open_footprint_bytes) = app
        .editor()
        .current_snapshot(buffer_id)
        .map(|descriptor| {
            (
                descriptor.line_count,
                descriptor.memory_footprint_bytes as u64,
            )
        })
        .unwrap_or((0, 0));

    let micros = elapsed.as_micros() as u64;
    let record = WorkloadRecord {
        name: "p8.startup",
        measured: true,
        // One sample on purpose: every later open in this process is warm, and
        // a warm open is not a startup.
        sample_count: 1,
        fixture_bytes: document_bytes,
        p50_micros: micros,
        p95_micros: micros,
        total_micros: micros,
        bytes_value: 0,
        // The phase breakdown is in the message on purpose: a single
        // open-to-ready number tells you a regression happened but not where,
        // and these three phases have nothing to do with each other.
        detail: format!(
            "open-to-ready on {} = {:.1}ms (AppComposition::new {:.1}ms, open_workspace {:.1}ms, \
             open_file({}) {:.1}ms, first projection {:.1}ms); first projection carried {} lines",
            workspace_root.display(),
            elapsed.as_secs_f64() * 1000.0,
            compose_elapsed.as_secs_f64() * 1000.0,
            (workspace_elapsed - compose_elapsed).as_secs_f64() * 1000.0,
            REFERENCE_DOCUMENT,
            (open_file_elapsed - workspace_elapsed).as_secs_f64() * 1000.0,
            (elapsed - open_file_elapsed).as_secs_f64() * 1000.0,
            projection.line_slices.len(),
        ),
    };
    (
        record,
        Some(ReadyWorkspace {
            app,
            document_bytes,
            line_count,
            open_footprint_bytes,
        }),
    )
}

fn viewport_request(
    buffer_id: legion_protocol::BufferId,
    top_line: usize,
) -> EditorViewportRequest {
    EditorViewportRequest {
        buffer_id,
        scroll: ViewportScroll {
            top_line: top_line as u32,
            left_column: 0,
        },
        dimensions: ViewportDimensions {
            width_px: VIEWPORT_WIDTH_PX,
            height_px: VIEWPORT_HEIGHT_PX,
        },
    }
}

/// Keystroke → projection, the real path, on a real document.
///
/// The insert and the projection are inside the timed region and the undoing
/// delete is outside it: the user pays for the keystroke they typed, not for
/// the harness putting the file back.
fn measure_input_to_paint(ready: &mut ReadyWorkspace) -> WorkloadRecord {
    let Some(buffer_id) = ready.app.active_buffer_id() else {
        return WorkloadRecord::unmeasured("p8.input_to_paint", "no active buffer");
    };
    // Type deep in the document rather than at line 0: an editor that is fast
    // only at the top of the file has not solved typing.
    let base_line = ready.line_count / 2;
    let mut samples = Vec::with_capacity(INPUT_SAMPLES);

    for index in 0..INPUT_SAMPLES {
        let line = base_line + index;
        if line >= ready.line_count {
            break;
        }
        let at = TextPosition::new(line, 0);
        let start = Instant::now();
        if let Err(err) = ready.app.edit_active_buffer(TextEdit::insert(at, "x")) {
            return WorkloadRecord::unmeasured(
                "p8.input_to_paint",
                format!("edit_active_buffer failed at line {line}: {err:?}"),
            );
        }
        // The frame the keystroke produces. Scrolled so the edited line is
        // inside the viewport, because a projection that does not contain the
        // edit is not the frame the user is waiting for.
        let top_line = line.saturating_sub(10);
        if let Err(err) = ready
            .app
            .editor()
            .viewport_projection(viewport_request(buffer_id, top_line))
        {
            return WorkloadRecord::unmeasured(
                "p8.input_to_paint",
                format!("viewport projection failed at line {line}: {err:?}"),
            );
        }
        samples.push(start.elapsed());

        if let Err(err) = ready
            .app
            .edit_active_buffer(TextEdit::delete(TextRange::new(
                at,
                TextPosition::new(at.line, at.column + 1),
            )))
        {
            return WorkloadRecord::unmeasured(
                "p8.input_to_paint",
                format!("undo delete failed at line {line}: {err:?}"),
            );
        }
    }

    finish_duration_workload(
        "p8.input_to_paint",
        samples,
        ready.document_bytes,
        format!(
            "real keystroke (edit_active_buffer) + real viewport projection on {REFERENCE_DOCUMENT} \
             ({} lines), typed at line {base_line}+",
            ready.line_count
        ),
    )
}

/// Viewport projections spread across the whole document.
fn measure_scroll(ready: &mut ReadyWorkspace) -> WorkloadRecord {
    let Some(buffer_id) = ready.app.active_buffer_id() else {
        return WorkloadRecord::unmeasured("p8.scroll_jank", "no active buffer");
    };
    if ready.line_count == 0 {
        return WorkloadRecord::unmeasured("p8.scroll_jank", "document has no lines");
    }
    let mut samples = Vec::with_capacity(SCROLL_SAMPLES);
    let stride = (ready.line_count / SCROLL_SAMPLES).max(1);
    for index in 0..SCROLL_SAMPLES {
        let top_line = (index * stride) % ready.line_count;
        let start = Instant::now();
        if let Err(err) = ready
            .app
            .editor()
            .viewport_projection(viewport_request(buffer_id, top_line))
        {
            return WorkloadRecord::unmeasured(
                "p8.scroll_jank",
                format!("viewport projection failed at top_line {top_line}: {err:?}"),
            );
        }
        samples.push(start.elapsed());
    }

    finish_duration_workload(
        "p8.scroll_jank",
        samples,
        ready.document_bytes,
        format!(
            "real viewport projections at {SCROLL_SAMPLES} scroll offsets spanning all {} lines \
             of {REFERENCE_DOCUMENT}",
            ready.line_count
        ),
    )
}

/// Snapshot footprint of a real document opened through the real open path.
fn measure_memory_ceiling(ready: &ReadyWorkspace) -> WorkloadRecord {
    let Some(buffer_id) = ready.app.active_buffer_id() else {
        return WorkloadRecord::unmeasured("p8.memory_ceiling", "no active buffer");
    };
    let descriptor = match ready.app.editor().current_snapshot(buffer_id) {
        Ok(descriptor) => descriptor,
        Err(err) => {
            return WorkloadRecord::unmeasured(
                "p8.memory_ceiling",
                format!("current_snapshot failed: {err:?}"),
            );
        }
    };
    let snapshot_bytes = descriptor.memory_footprint_bytes as u64;
    let retained_bytes = ready.app.editor().retained_snapshot_estimated_bytes() as u64;
    let total = snapshot_bytes.saturating_add(retained_bytes);

    WorkloadRecord {
        name: "p8.memory_ceiling",
        measured: true,
        sample_count: 1,
        fixture_bytes: ready.document_bytes,
        p50_micros: 0,
        p95_micros: 0,
        total_micros: 0,
        bytes_value: total,
        detail: format!(
            "real document {REFERENCE_DOCUMENT} ({} bytes on disk, {} lines) after the typing \
             workload: snapshot footprint {snapshot_bytes} bytes + retained snapshot descriptors \
             {retained_bytes} bytes; at-open footprint was {} bytes",
            ready.document_bytes, ready.line_count, ready.open_footprint_bytes,
        ),
    }
}

/// Join the background search worker. Dispatch now returns a running
/// projection immediately; the planted-needle check has to wait for the
/// walk, or it reports 0 hits and the harness treats the row as unmeasured.
fn settle_search(app: &mut AppComposition, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        app.drain_search_worker();
        if !app.search_worker_in_flight() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    app.drain_search_worker();
}

fn search_hit_count(app: &AppComposition) -> usize {
    app.shell_projection_snapshot("product-perf")
        .map(|snapshot| snapshot.search_projection.results.len())
        .unwrap_or(0)
}

/// Real product workspace search over this repository.
fn measure_legion_repo_search(ready: &mut ReadyWorkspace) -> WorkloadRecord {
    let start = Instant::now();
    match ready
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RunSearch {
            scope: SearchScopeProjection::Workspace,
            query: REPO_NEEDLE.to_string(),
            // High enough that the walker cannot stop early: the workload is a
            // full-repo scan, and a search that quits at 50 hits measures nothing.
            limit: 100_000,
            case_sensitive: Some(true),
            whole_word: None,
            use_regex: None,
        }) {
        Ok(AppCommandOutcome::SearchUpdated(_)) => {}
        Ok(other) => {
            return WorkloadRecord::unmeasured(
                "p8.legion_repo",
                format!("expected SearchUpdated, got {other:?}"),
            );
        }
        Err(err) => {
            return WorkloadRecord::unmeasured(
                "p8.legion_repo",
                format!("RunSearch dispatch failed: {err:?}"),
            );
        }
    }
    settle_search(&mut ready.app, Duration::from_secs(120));
    let elapsed = start.elapsed();
    let hits = search_hit_count(&ready.app);
    if hits == 0 {
        // Zero hits means the walk never read this file's contents, so the
        // number would describe a directory listing rather than a search.
        return WorkloadRecord::unmeasured(
            "p8.legion_repo",
            format!("repo search found 0 hits for the planted needle after {elapsed:?}"),
        );
    }

    let micros = elapsed.as_micros() as u64;
    WorkloadRecord {
        name: "p8.legion_repo",
        measured: true,
        sample_count: 1,
        fixture_bytes: 0,
        p50_micros: micros,
        p95_micros: micros,
        total_micros: micros,
        bytes_value: 0,
        detail: format!(
            "real product RunSearch over the Legion repository: {:.0}ms, {hits} hit(s) for the \
             planted needle",
            elapsed.as_secs_f64() * 1000.0
        ),
    }
}

/// Generate the 100K-file reference workspace and search it through the real
/// product path.
fn measure_fixture_100k() -> WorkloadRecord {
    let root = std::env::temp_dir().join(format!("legion-perf-100k-{}", std::process::id()));
    let generate_start = Instant::now();
    if let Err(err) = generate_fixture(&root) {
        let _ = std::fs::remove_dir_all(&root);
        return WorkloadRecord::unmeasured(
            "p8.fixture_100k_files",
            format!("fixture generation failed: {err}"),
        );
    }
    let generate_elapsed = generate_start.elapsed();

    let open_start = Instant::now();
    let mut app = AppComposition::new();
    if let Err(err) = app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("perf-harness".to_string()),
    ) {
        let _ = std::fs::remove_dir_all(&root);
        return WorkloadRecord::unmeasured(
            "p8.fixture_100k_files",
            format!("open_workspace on 100K fixture failed: {err:?}"),
        );
    }
    // Reported separately from the search: opening a 100K-file workspace and
    // searching one are different questions, and a single number would not say
    // which of them moved.
    let open_elapsed = open_start.elapsed();

    let start = Instant::now();
    match app.dispatch_ui_intent(CommandDispatchIntent::RunSearch {
        scope: SearchScopeProjection::Workspace,
        query: FIXTURE_NEEDLE.to_string(),
        limit: 100_000,
        case_sensitive: Some(true),
        whole_word: None,
        use_regex: None,
    }) {
        Ok(AppCommandOutcome::SearchUpdated(_)) => {}
        Ok(other) => {
            let _ = std::fs::remove_dir_all(&root);
            return WorkloadRecord::unmeasured(
                "p8.fixture_100k_files",
                format!("expected SearchUpdated, got {other:?}"),
            );
        }
        Err(err) => {
            let _ = std::fs::remove_dir_all(&root);
            return WorkloadRecord::unmeasured(
                "p8.fixture_100k_files",
                format!("RunSearch dispatch failed: {err:?}"),
            );
        }
    }
    settle_search(&mut app, Duration::from_secs(1_800));
    let elapsed = start.elapsed();
    let hits = search_hit_count(&app);
    let _ = std::fs::remove_dir_all(&root);

    if hits == 0 {
        return WorkloadRecord::unmeasured(
            "p8.fixture_100k_files",
            format!("100K-file search found 0 hits for the planted needle after {elapsed:?}"),
        );
    }

    let micros = elapsed.as_micros() as u64;
    WorkloadRecord {
        name: "p8.fixture_100k_files",
        measured: true,
        sample_count: 1,
        fixture_bytes: FIXTURE_FILE_COUNT as u64,
        p50_micros: micros,
        p95_micros: micros,
        total_micros: micros,
        bytes_value: 0,
        detail: format!(
            "real product RunSearch over {FIXTURE_FILE_COUNT} generated files: {:.0}ms, {hits} \
             hit(s); workspace open {:.0}ms and fixture generation {:.0}ms are both excluded \
             from the measured region",
            elapsed.as_secs_f64() * 1000.0,
            open_elapsed.as_secs_f64() * 1000.0,
            generate_elapsed.as_secs_f64() * 1000.0,
        ),
    }
}

fn generate_fixture(root: &Path) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|err| format!("create {}: {err}", root.display()))?;
    let dir_count = FIXTURE_FILE_COUNT.div_ceil(FIXTURE_FILES_PER_DIR);
    // The needle goes in exactly one file, in the last directory the walker
    // reaches, so a search that finds it necessarily walked the whole tree.
    let needle_index = FIXTURE_FILE_COUNT - 1;
    for dir_index in 0..dir_count {
        let dir = root.join(format!("pkg{dir_index:04}"));
        std::fs::create_dir_all(&dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
        for file_index in 0..FIXTURE_FILES_PER_DIR {
            let global = dir_index * FIXTURE_FILES_PER_DIR + file_index;
            if global >= FIXTURE_FILE_COUNT {
                break;
            }
            let body = if global == needle_index {
                format!(
                    "// generated module {global}\npub const MARK: &str = \"{FIXTURE_NEEDLE}\";\n"
                )
            } else {
                format!(
                    "// generated module {global}\npub fn item_{global}() -> usize {{ {global} }}\n"
                )
            };
            std::fs::write(dir.join(format!("mod_{file_index:04}.rs")), body)
                .map_err(|err| format!("write fixture file {global}: {err}"))?;
        }
    }
    Ok(())
}

fn finish_duration_workload(
    name: &'static str,
    mut samples: Vec<Duration>,
    fixture_bytes: u64,
    detail: String,
) -> WorkloadRecord {
    if samples.is_empty() {
        return WorkloadRecord::unmeasured(name, "no samples collected");
    }
    let total: Duration = samples.iter().copied().sum();
    let sample_count = samples.len();
    samples.sort();
    WorkloadRecord {
        name,
        measured: true,
        sample_count,
        fixture_bytes,
        p50_micros: percentile(&samples, 0.50).as_micros() as u64,
        p95_micros: percentile(&samples, 0.95).as_micros() as u64,
        total_micros: total.as_micros() as u64,
        bytes_value: 0,
        detail,
    }
}

fn percentile(sorted: &[Duration], quantile: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let index = ((sorted.len() as f64 - 1.0) * quantile).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

fn render_report(workspace_root: &Path, records: &[WorkloadRecord]) -> String {
    let mut out = String::new();
    out.push_str("schema_version = 1\n");
    out.push_str(&format!("os = \"{}\"\n", std::env::consts::OS));
    out.push_str(&format!("arch = \"{}\"\n", std::env::consts::ARCH));
    out.push_str(&format!(
        "workspace_root = \"{}\"\n",
        toml_escape(&workspace_root.to_string_lossy())
    ));
    for record in records {
        out.push_str("\n[[workload]]\n");
        out.push_str(&format!("name = \"{}\"\n", record.name));
        out.push_str(&format!("measured = {}\n", record.measured));
        out.push_str(&format!("sample_count = {}\n", record.sample_count));
        out.push_str(&format!("fixture_bytes = {}\n", record.fixture_bytes));
        out.push_str(&format!("p50_micros = {}\n", record.p50_micros));
        out.push_str(&format!("p95_micros = {}\n", record.p95_micros));
        out.push_str(&format!("total_micros = {}\n", record.total_micros));
        out.push_str(&format!("bytes_value = {}\n", record.bytes_value));
        out.push_str(&format!("detail = \"{}\"\n", toml_escape(&record.detail)));
    }
    out
}

fn toml_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r'], " ")
}
