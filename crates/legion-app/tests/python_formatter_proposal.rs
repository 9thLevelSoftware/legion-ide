//! Component coverage for the external Python formatter proposal route.
//!
//! Every executable here is a fixture file the test creates and every process
//! result comes from the inline fake below. Nothing in this file depends on
//! `black`, `ruff`, `python`, or any real formatter being installed, and no
//! child process is ever spawned: `FakeProcess` is the whole process authority.
//! These are therefore **component** assertions about the route, not evidence
//! that a real formatter produces correct Python.

use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

use legion_app::language::{
    ExternalFormatterBounds, ExternalFormatterError, ExternalFormatterRun,
    PYTHON_FORMATTER_EXPECTED_EXIT_CODE, PYTHON_FORMATTER_MAX_DOCUMENT_BYTES,
    PYTHON_FORMATTER_STDERR_LIMIT, PYTHON_FORMATTER_STDIN_ARGS, PYTHON_FORMATTER_STDOUT_LIMIT,
    PYTHON_FORMATTER_TIMEOUT, run_external_formatter,
};
use legion_app::{AppCommandOutcome, AppComposition};
use legion_editor::{TextEdit, TextPosition};
use legion_platform::{
    BoundedProcessRequest, MAX_BOUNDED_STDIN_BYTES, PlatformError, ProcessRequest, ProcessResult,
    ProcessService,
};
use legion_protocol::{
    BufferId, CapabilityBrokerPort, CapabilityDecision, CapabilityDecisionId, CapabilityDenial,
    CapabilityId, CapabilityRequest, CapabilityResponse, CausalityId, CorrelationId,
    LanguageToolingOperationKind, LanguageToolingProjection, LanguageToolingStatusKind,
    PrincipalId, ProtocolResult, WorkspaceId, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;
use uuid::Uuid;

/// Unformatted seed. Deliberately valid Python so a `Failed` outcome can never
/// be blamed on the input.
const UNFORMATTED: &str = "def greet( name ):\n  return  name\n";

/// What the fake formatter prints on stdout when it "succeeds".
const FORMATTED: &str = "def greet(name):\n    return name\n";

/// Marker printed on stderr by every fake run. It must never appear in an
/// operation message, a projection message, or a proposal.
const STDERR_MARKER: &str = "FORMATTER-STDERR-MUST-NOT-LEAK-9f13c2";

// ---------------------------------------------------------------------------
// Inline fakes, modelled on `tests/formatter_executable_approval.rs`.
// ---------------------------------------------------------------------------

struct FakeBroker {
    response: Mutex<Option<CapabilityResponse>>,
    requests: AtomicUsize,
}

impl FakeBroker {
    fn granted() -> Self {
        Self {
            response: Mutex::new(Some(CapabilityResponse::Decision(CapabilityDecision {
                decision_id: CapabilityDecisionId(9),
                granted: true,
                capability: CapabilityId("lsp.launch".to_string()),
                reason: None,
            }))),
            requests: AtomicUsize::new(0),
        }
    }

    fn denied() -> Self {
        Self {
            response: Mutex::new(Some(CapabilityResponse::Denied(CapabilityDenial {
                decision_id: CapabilityDecisionId(3),
                principal_id: PrincipalId("external-formatter-tests".to_string()),
                capability_id: CapabilityId("lsp.launch".to_string()),
                reason: "formatter is not on the operator allowlist".to_string(),
            }))),
            requests: AtomicUsize::new(0),
        }
    }
}

impl CapabilityBrokerPort for FakeBroker {
    fn handle(&self, _request: CapabilityRequest) -> ProtocolResult<CapabilityResponse> {
        self.requests.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .response
            .lock()
            .unwrap()
            .take()
            .expect("one broker response per run"))
    }
}

struct FakeProcess {
    calls: AtomicUsize,
    request: Mutex<Option<BoundedProcessRequest>>,
    result: Mutex<ProcessResult>,
    /// Raised from inside `execute_bounded`, so the route must observe a cancel
    /// that happened while the child was running.
    cancel_before_return: Mutex<Option<Arc<AtomicBool>>>,
}

impl FakeProcess {
    fn new(exit_code: i32, stdout: &str) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            request: Mutex::new(None),
            result: Mutex::new(ProcessResult {
                exit_code,
                stdout: stdout.to_string(),
                stderr: STDERR_MARKER.to_string(),
                elapsed: Duration::from_millis(1),
            }),
            cancel_before_return: Mutex::new(None),
        }
    }

    fn cancelling(exit_code: i32, stdout: &str, flag: Arc<AtomicBool>) -> Self {
        let process = Self::new(exit_code, stdout);
        *process.cancel_before_return.lock().unwrap() = Some(flag);
        process
    }

    fn spawns(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn recorded(&self) -> BoundedProcessRequest {
        self.request
            .lock()
            .unwrap()
            .clone()
            .expect("a bounded request was recorded")
    }
}

impl ProcessService for FakeProcess {
    fn execute(&self, _request: &ProcessRequest) -> Result<ProcessResult, PlatformError> {
        panic!("the external formatter route must never use unbounded execute()");
    }

    fn execute_bounded(
        &self,
        request: &BoundedProcessRequest,
    ) -> Result<ProcessResult, PlatformError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.request.lock().unwrap() = Some(request.clone());
        if let Some(flag) = self.cancel_before_return.lock().unwrap().as_ref() {
            flag.store(true, Ordering::SeqCst);
        }
        Ok(self.result.lock().unwrap().clone())
    }
}

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

struct Fixture {
    _root: tempfile::TempDir,
    app: AppComposition,
    buffer: BufferId,
    source: PathBuf,
    formatter: PathBuf,
}

/// Builds a trusted workspace with one Python buffer, optionally configuring an
/// operator-selected Python toolchain whose executables are fixture files.
fn fixture(configure_python: bool) -> Fixture {
    let root = tempfile::tempdir().expect("temporary workspace");
    let source = root.path().join("main.py");
    std::fs::write(&source, UNFORMATTED).expect("seed Python source");
    let tools = root.path().join("tools");
    std::fs::create_dir_all(&tools).expect("tool directory");
    let interpreter = tools.join("python-fixture");
    let formatter = tools.join("formatter-fixture");
    std::fs::write(&interpreter, b"not a real interpreter").expect("interpreter fixture");
    std::fs::write(&formatter, b"not a real formatter").expect("formatter fixture");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("external-formatter-tests".to_string()),
    )
    .expect("open workspace");
    if configure_python {
        app.configure_python_toolchain(&interpreter, &formatter)
            .expect("configure Python toolchain");
    }
    app.open_file(source.to_string_lossy())
        .expect("open Python file");
    let buffer = app.active_buffer_id().expect("active Python buffer");

    Fixture {
        _root: root,
        app,
        buffer,
        source,
        formatter,
    }
}

fn formatting_rows(
    projection: &LanguageToolingProjection,
) -> Vec<legion_protocol::LanguageToolingOperationProjection> {
    projection
        .operations
        .iter()
        .filter(|operation| operation.kind == LanguageToolingOperationKind::FormattingProposal)
        .cloned()
        .collect()
}

fn sole_formatting_row(
    app: &AppComposition,
) -> legion_protocol::LanguageToolingOperationProjection {
    let projection = app.language_tooling_projection();
    let rows = formatting_rows(&projection);
    assert_eq!(
        rows.len(),
        1,
        "exactly one formatting operation row is expected, got {rows:?}"
    );
    rows.into_iter().next().expect("formatting row")
}

fn canonical(path: &Path) -> String {
    std::fs::canonicalize(path)
        .expect("canonicalize fixture")
        .to_str()
        .expect("fixture path is UTF-8")
        .to_string()
}

fn flag() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

fn free_run(
    document: &str,
    bounds: ExternalFormatterBounds,
    executable: PathBuf,
) -> ExternalFormatterRun {
    ExternalFormatterRun {
        executable,
        args: PYTHON_FORMATTER_STDIN_ARGS
            .iter()
            .map(|arg| arg.to_string())
            .collect(),
        expected_exit_code: PYTHON_FORMATTER_EXPECTED_EXIT_CODE,
        document: document.to_string(),
        bounds,
        principal_id: PrincipalId("external-formatter-tests".to_string()),
        workspace_id: WorkspaceId(42),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        correlation_id: CorrelationId(77),
        causality_id: CausalityId(Uuid::from_u128(7)),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn unconfigured_python_formatter_records_failed_operation_and_no_proposal() {
    let mut fx = fixture(false);
    let broker = FakeBroker::granted();
    let process = FakeProcess::new(0, FORMATTED);

    // With no configured formatter the route declines the buffer outright, even
    // with both ports available: no approval is asked for and no child is
    // spawned.
    assert!(
        !fx.app.run_external_python_formatting_with_ports_for_test(
            fx.buffer,
            &broker,
            &process,
            flag()
        ),
        "an unconfigured buffer is not this route's to answer"
    );
    assert_eq!(process.spawns(), 0);
    assert_eq!(broker.requests.load(Ordering::SeqCst), 0);
    assert!(
        formatting_rows(&fx.app.language_tooling_projection()).is_empty(),
        "declining the route must not record an operation of its own"
    );

    // The product request then reaches the language-server route, which has no
    // live capable server, and that is recorded as an explicit failure with no
    // proposal — never as a fabricated or empty one.
    let outcome = fx
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestFormattingProposal {
            buffer_id: fx.buffer,
        })
        .expect("formatting dispatch");
    let projection = match outcome {
        AppCommandOutcome::LanguageToolingUpdated(projection) => *projection,
        other => panic!("expected a language tooling projection, got {other:?}"),
    };
    assert_eq!(projection.status, LanguageToolingStatusKind::Failed);
    let rows = formatting_rows(&projection);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, LanguageToolingStatusKind::Failed);
    assert!(rows[0].proposal_id.is_none(), "no proposal may be minted");
}

#[test]
fn configured_but_unapproved_formatter_never_spawns_and_records_failed_operation() {
    let mut fx = fixture(true);
    let broker = FakeBroker::denied();
    let process = FakeProcess::new(0, FORMATTED);

    assert!(
        fx.app.run_external_python_formatting_with_ports_for_test(
            fx.buffer,
            &broker,
            &process,
            flag()
        ),
        "a configured Python formatter is answered by this route"
    );

    assert_eq!(
        process.spawns(),
        0,
        "a denied capability decision must stop before any child process exists"
    );
    assert_eq!(
        broker.requests.load(Ordering::SeqCst),
        1,
        "fresh authority is requested at run time rather than assumed"
    );
    let row = sole_formatting_row(&fx.app);
    assert_eq!(row.status, LanguageToolingStatusKind::Failed);
    assert!(row.proposal_id.is_none());
}

#[test]
fn exact_command_args_stdin_and_bounds_are_forwarded_to_the_process_service() {
    let mut fx = fixture(true);
    let broker = FakeBroker::granted();
    let process = FakeProcess::new(0, FORMATTED);

    assert!(fx.app.run_external_python_formatting_with_ports_for_test(
        fx.buffer,
        &broker,
        &process,
        flag()
    ));

    assert_eq!(
        process.spawns(),
        1,
        "the formatting run is the only child; approval spawns no probe of its own"
    );
    let request = process.recorded();
    assert_eq!(request.process.command, canonical(&fx.formatter));
    assert_eq!(request.process.args, vec!["-".to_string()]);
    assert_eq!(
        request.process.args,
        PYTHON_FORMATTER_STDIN_ARGS
            .iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<_>>(),
        "the argument vector is the documented constant, not a per-binary guess"
    );
    assert_eq!(
        request.process.stdin.as_deref(),
        Some(UNFORMATTED.as_bytes()),
        "the buffer snapshot is delivered on stdin"
    );
    assert_eq!(request.process.cwd, None);
    assert!(request.process.env.is_empty());
    assert!(!request.process.cancelled);
    assert_eq!(request.max_stdout_bytes, PYTHON_FORMATTER_STDOUT_LIMIT);
    assert_eq!(request.max_stderr_bytes, PYTHON_FORMATTER_STDERR_LIMIT);
    assert!(
        request.timeout > Duration::ZERO && request.timeout <= PYTHON_FORMATTER_TIMEOUT,
        "the child receives what is left of the single finite budget, got {:?}",
        request.timeout
    );
    assert_eq!(request.process.timeout, Some(request.timeout));
    assert!(
        !request
            .process
            .args
            .iter()
            .any(|arg| arg.contains("main.py")),
        "the workspace path is never handed to the child"
    );
}

#[test]
fn buffer_snapshot_is_delivered_on_stdin_and_nothing_is_written_to_disk() {
    let mut fx = fixture(true);
    let on_disk_before = std::fs::read(&fx.source).expect("read seed from disk");

    // Make the buffer dirty so the in-memory snapshot and the file on disk
    // differ: the formatter must see the snapshot the proposal will be diffed
    // against, not the stale bytes on disk.
    fx.app
        .edit_active_buffer(TextEdit::insert(TextPosition::new(0, 0), "# dirty\n"))
        .expect("make the buffer dirty");
    let dirty_text = fx
        .app
        .buffer_text_for_input(fx.buffer)
        .expect("dirty buffer text");
    assert_ne!(dirty_text.as_bytes(), on_disk_before.as_slice());

    let broker = FakeBroker::granted();
    let process = FakeProcess::new(0, FORMATTED);
    assert!(fx.app.run_external_python_formatting_with_ports_for_test(
        fx.buffer,
        &broker,
        &process,
        flag()
    ));

    let request = process.recorded();
    assert_eq!(
        request.process.stdin.as_deref(),
        Some(dirty_text.as_bytes()),
        "the in-memory snapshot goes on stdin, never the file on disk"
    );
    assert_eq!(
        std::fs::read(&fx.source).expect("read after run"),
        on_disk_before,
        "OR-DISK-UNCHANGED-ON-CANCEL: proposing must not touch the workspace file"
    );
    assert_eq!(
        fx.app
            .buffer_text_for_input(fx.buffer)
            .expect("buffer text after run"),
        dirty_text,
        "the editor buffer is not rewritten by a proposal"
    );
    assert!(
        fx.app
            .editor()
            .is_dirty(fx.buffer)
            .expect("dirty state after run"),
        "the buffer's dirty state is unchanged"
    );
    // No temporary spool of the document may appear anywhere under the
    // workspace root.
    let entries: Vec<String> = std::fs::read_dir(fx._root.path())
        .expect("read workspace root")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert!(
        !entries.iter().any(|name| name.contains("main.py.")),
        "no temp file beside the source: {entries:?}"
    );
}

#[test]
fn nonzero_exit_code_discards_stdout_and_records_a_failed_operation() {
    let mut fx = fixture(true);
    let broker = FakeBroker::granted();
    // Plausible, valid, formatted-looking Python on stdout — the point is that
    // the exit status alone decides whether it is ever looked at.
    let process = FakeProcess::new(1, FORMATTED);

    assert!(fx.app.run_external_python_formatting_with_ports_for_test(
        fx.buffer,
        &broker,
        &process,
        flag()
    ));

    let row = sole_formatting_row(&fx.app);
    assert_eq!(row.status, LanguageToolingStatusKind::Failed);
    assert!(
        row.proposal_id.is_none(),
        "stdout from a non-zero exit must never become a proposal"
    );
    assert!(
        !row.message.contains("def greet"),
        "the discarded stdout must not surface in the operation message: {}",
        row.message
    );
    assert_eq!(
        fx.app
            .buffer_text_for_input(fx.buffer)
            .expect("buffer text"),
        UNFORMATTED
    );
}

#[test]
fn formatted_stdout_becomes_a_reviewable_proposal_leaving_editor_and_disk_unchanged() {
    let mut fx = fixture(true);
    let on_disk_before = std::fs::read(&fx.source).expect("read seed from disk");
    let broker = FakeBroker::granted();
    let process = FakeProcess::new(0, FORMATTED);

    assert!(fx.app.run_external_python_formatting_with_ports_for_test(
        fx.buffer,
        &broker,
        &process,
        flag()
    ));

    let row = sole_formatting_row(&fx.app);
    assert_eq!(row.status, LanguageToolingStatusKind::Ready);
    let proposal_id = row.proposal_id.expect("a reviewable proposal was minted");
    let proposal = fx
        .app
        .workspace_proposal_for_id(proposal_id)
        .expect("proposal is registered with the coordinator");
    let encoded = serde_json::to_string(&proposal).expect("serialize proposal");
    assert!(
        encoded.contains("def greet(name):"),
        "the proposal must carry the formatter's own stdout"
    );
    assert!(
        proposal.preview.summary.contains("formatter-fixture"),
        "the title names the tool that produced the edit: {}",
        proposal.preview.summary
    );
    assert!(
        proposal
            .preview
            .details
            .iter()
            .any(|detail| detail == "language_tooling.external_formatter"),
        "the detail tag records external provenance: {:?}",
        proposal.preview.details
    );

    assert_eq!(
        std::fs::read(&fx.source).expect("read after run"),
        on_disk_before,
        "a proposal must leave the workspace file byte-identical"
    );
    assert_eq!(
        fx.app
            .buffer_text_for_input(fx.buffer)
            .expect("buffer text after run"),
        UNFORMATTED,
        "a proposal must leave the editor buffer unchanged"
    );
    assert!(
        !fx.app
            .editor()
            .is_dirty(fx.buffer)
            .expect("dirty state after run"),
        "a proposal must not dirty a clean buffer"
    );
}

#[test]
fn unchanged_formatter_output_records_a_no_op_without_minting_an_empty_proposal() {
    let mut fx = fixture(true);
    let broker = FakeBroker::granted();
    // The formatter succeeded and reported the document is already formatted.
    let process = FakeProcess::new(0, UNFORMATTED);

    assert!(fx.app.run_external_python_formatting_with_ports_for_test(
        fx.buffer,
        &broker,
        &process,
        flag()
    ));

    let row = sole_formatting_row(&fx.app);
    assert_ne!(
        row.status,
        LanguageToolingStatusKind::Running,
        "the operation must reach a terminal status"
    );
    assert!(
        row.proposal_id.is_none(),
        "an unchanged document must not mint a proposal with no edits"
    );
}

#[test]
fn stale_snapshot_between_request_and_result_records_stale_and_no_proposal() {
    let mut fx = fixture(true);
    let admission = fx
        .app
        .admit_external_python_formatting_for_test(fx.buffer)
        .expect("a configured Python buffer is admitted");

    // The user keeps typing while the formatter runs.
    fx.app
        .edit_active_buffer(TextEdit::insert(TextPosition::new(0, 0), "# moved on\n"))
        .expect("move the snapshot on");
    let text_after_edit = fx
        .app
        .buffer_text_for_input(fx.buffer)
        .expect("buffer text after edit");

    assert!(
        fx.app
            .complete_external_python_formatting_for_test(admission, Ok(FORMATTED.to_string()))
    );

    let row = sole_formatting_row(&fx.app);
    assert_eq!(row.status, LanguageToolingStatusKind::Stale);
    assert!(
        row.proposal_id.is_none(),
        "a result against a moved snapshot must never become a proposal"
    );
    assert_eq!(
        fx.app
            .buffer_text_for_input(fx.buffer)
            .expect("buffer text after stale result"),
        text_after_edit
    );
}

#[test]
fn cancellation_terminates_the_child_and_records_no_proposal() {
    let mut fx = fixture(true);
    let cancellation = flag();
    let broker = FakeBroker::granted();
    // The child "runs" and the cancel is raised while it is running; it then
    // returns a perfectly good exit status and stdout.
    let process = FakeProcess::cancelling(0, FORMATTED, Arc::clone(&cancellation));

    assert!(fx.app.run_external_python_formatting_with_ports_for_test(
        fx.buffer,
        &broker,
        &process,
        Arc::clone(&cancellation)
    ));

    let request = process.recorded();
    assert!(
        Arc::ptr_eq(&request.cancellation, &cancellation),
        "the flag handed to the bounded runner must be the one a cancel flips; \
         terminating the child is that runner's job and it can only do it with this flag"
    );
    assert!(cancellation.load(Ordering::SeqCst));
    let row = sole_formatting_row(&fx.app);
    assert_eq!(row.status, LanguageToolingStatusKind::Cancelled);
    assert!(
        row.proposal_id.is_none(),
        "a cancelled run must not leave a proposal behind"
    );
    assert_eq!(
        fx.app
            .buffer_text_for_input(fx.buffer)
            .expect("buffer text"),
        UNFORMATTED
    );
}

#[test]
fn buffer_larger_than_the_bounded_stdin_cap_fails_closed_before_spawn() {
    let fx = fixture(true);
    let broker = FakeBroker::granted();
    let process = FakeProcess::new(0, FORMATTED);

    // The cap is a field of the run, so the same production check is exercised
    // with a small injected bound instead of allocating 4 MiB of text.
    let bounds = ExternalFormatterBounds {
        max_document_bytes: 16,
        ..ExternalFormatterBounds::PYTHON
    };
    let document = "x".repeat(17);
    let run = free_run(&document, bounds, fx.formatter.clone());

    let error = run_external_formatter(&broker, &process, &run, flag())
        .expect_err("an over-size document must fail closed");
    match error {
        ExternalFormatterError::DocumentTooLarge { actual, limit } => {
            assert_eq!(actual, 17);
            assert_eq!(limit, 16);
        }
        other => panic!("expected DocumentTooLarge, got {other:?}"),
    }
    assert_eq!(
        process.spawns(),
        0,
        "no child may exist after an over-size rejection"
    );
    assert_eq!(
        broker.requests.load(Ordering::SeqCst),
        0,
        "an over-size document must not even cause a capability decision"
    );

    // The production bound must stay inside the transport limit, otherwise the
    // route would hand the process service a payload it rejects.
    const { assert!(PYTHON_FORMATTER_MAX_DOCUMENT_BYTES <= MAX_BOUNDED_STDIN_BYTES) };
    assert_eq!(
        ExternalFormatterBounds::PYTHON.max_document_bytes,
        PYTHON_FORMATTER_MAX_DOCUMENT_BYTES
    );
}

#[test]
fn formatter_stderr_never_reaches_the_proposal_or_the_operation_message() {
    // Success path: stderr is present on every fake result and must not appear
    // in the proposal or in any recorded message.
    let mut fx = fixture(true);
    let broker = FakeBroker::granted();
    let process = FakeProcess::new(0, FORMATTED);
    assert!(fx.app.run_external_python_formatting_with_ports_for_test(
        fx.buffer,
        &broker,
        &process,
        flag()
    ));
    let projection = fx.app.language_tooling_projection();
    let row = sole_formatting_row(&fx.app);
    assert!(!row.message.contains(STDERR_MARKER));
    assert!(!projection.status_message.contains(STDERR_MARKER));
    let proposal = fx
        .app
        .workspace_proposal_for_id(row.proposal_id.expect("proposal"))
        .expect("proposal is registered");
    let encoded = serde_json::to_string(&proposal).expect("serialize proposal");
    assert!(
        !encoded.contains(STDERR_MARKER),
        "no proposal field may carry child stderr"
    );

    // Failure path: the operation message explains the failure without quoting
    // anything the child printed.
    let mut failed = fixture(true);
    let broker = FakeBroker::granted();
    let process = FakeProcess::new(2, FORMATTED);
    assert!(
        failed
            .app
            .run_external_python_formatting_with_ports_for_test(
                failed.buffer,
                &broker,
                &process,
                flag()
            )
    );
    let failed_projection = failed.app.language_tooling_projection();
    let failed_row = sole_formatting_row(&failed.app);
    assert_eq!(failed_row.status, LanguageToolingStatusKind::Failed);
    assert!(!failed_row.message.contains(STDERR_MARKER));
    assert!(!failed_projection.status_message.contains(STDERR_MARKER));
}

#[test]
fn configured_python_formatter_takes_precedence_and_a_typescript_buffer_still_routes_to_lsp() {
    let root = tempfile::tempdir().expect("temporary workspace");
    let typescript = root.path().join("app.ts");
    let python = root.path().join("main.py");
    std::fs::write(&typescript, "export const x =   1\n").expect("seed TypeScript source");
    std::fs::write(&python, UNFORMATTED).expect("seed Python source");
    let tools = root.path().join("tools");
    std::fs::create_dir_all(&tools).expect("tool directory");
    let interpreter = tools.join("python-fixture");
    let formatter = tools.join("formatter-fixture");
    std::fs::write(&interpreter, b"not a real interpreter").expect("interpreter fixture");
    std::fs::write(&formatter, b"not a real formatter").expect("formatter fixture");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("external-formatter-tests".to_string()),
    )
    .expect("open workspace");
    app.configure_python_toolchain(&interpreter, &formatter)
        .expect("configure Python toolchain");
    app.open_file(typescript.to_string_lossy())
        .expect("open TypeScript file");
    let typescript_buffer = app.active_buffer_id().expect("TypeScript buffer");
    app.open_file(python.to_string_lossy())
        .expect("open Python file");
    let python_buffer = app.active_buffer_id().expect("Python buffer");
    assert_ne!(typescript_buffer, python_buffer);

    let broker = FakeBroker::granted();
    let process = FakeProcess::new(0, FORMATTED);

    // Direction one: the TypeScript buffer is refused by the external route
    // even with both ports available, and the language-server formatting
    // request answers it exactly as it did before this route existed.
    assert!(
        !app.run_external_python_formatting_with_ports_for_test(
            typescript_buffer,
            &broker,
            &process,
            flag()
        ),
        "a TypeScript buffer is never routed to the Python formatter"
    );
    assert_eq!(process.spawns(), 0);
    assert_eq!(broker.requests.load(Ordering::SeqCst), 0);
    assert!(
        formatting_rows(&app.language_tooling_projection()).is_empty(),
        "declining must not record an operation"
    );
    assert!(
        !app.issue_lsp_formatting_request(typescript_buffer),
        "with no live capable server the TypeScript request is unavailable, as before"
    );

    // Direction two: the Python buffer takes the external route and produces a
    // reviewable proposal from the formatter's own stdout.
    assert!(app.run_external_python_formatting_with_ports_for_test(
        python_buffer,
        &broker,
        &process,
        flag()
    ));
    assert_eq!(process.spawns(), 1);
    let rows = formatting_rows(&app.language_tooling_projection());
    assert_eq!(rows.len(), 1, "only the Python buffer recorded a run");
    assert_eq!(rows[0].status, LanguageToolingStatusKind::Ready);
    assert!(rows[0].proposal_id.is_some());
    assert_eq!(
        std::fs::read_to_string(&python).expect("read Python source"),
        UNFORMATTED,
        "the proposal left the file on disk untouched"
    );
    assert_eq!(
        std::fs::read_to_string(&typescript).expect("read TypeScript source"),
        "export const x =   1\n",
        "the TypeScript file was never touched"
    );
}
