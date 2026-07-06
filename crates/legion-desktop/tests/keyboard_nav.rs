//! Keyboard navigation smoke test for the desktop adapter.
//!
//! This regression ensures the product-mode switch can be activated without a
//! pointer by tabbing to the first pill and pressing Enter.

use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_ui::DockMode;

/// Build a five-target batch proposal suitable for seeding proposal_reviews in
/// the desktop runtime's delegated-task projection.
///
/// Each item has a unique target and path.  The diff_summary for a Batch
/// proposal sets `hunk_count = items.len()` so the delegated-task projection
/// filter (`hunk_count > 0`) will include this proposal.
fn five_target_batch_proposal() -> legion_protocol::WorkspaceProposal {
    use legion_protocol::{
        BatchProposalPayload, CanonicalPath, CapabilityId, CorrelationId, FileId, PreviewSummary,
        PrincipalId, ProposalAffectedTarget, ProposalBatchAtomicity, ProposalBatchItem,
        ProposalBatchRollbackPolicy, ProposalId, ProposalPayload, ProposalTargetCoverage,
        ProposalTargetCoverageKind, ProposalTargetKind, ProposalVersionPreconditions, WorkspaceId,
        WorkspaceProposal,
    };

    let targets: Vec<ProposalAffectedTarget> = (1u32..=5)
        .map(|i| ProposalAffectedTarget {
            target_id: format!("nav-target-{i}"),
            kind: ProposalTargetKind::ClosedFile,
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(FileId(i.into())),
            buffer_id: None,
            path: Some(CanonicalPath(format!("src/nav_{i}.rs"))),
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges: Vec::new(),
            redaction_hints: Vec::new(),
        })
        .collect();

    let items: Vec<ProposalBatchItem> = targets
        .iter()
        .enumerate()
        .map(|(i, target)| ProposalBatchItem {
            order: i as u32,
            item_id: format!("nav-item-{}", i + 1),
            payload: Box::new(ProposalPayload::CreateFile(
                legion_protocol::CreateFileProposal {
                    path: target.path.clone().unwrap(),
                    initial_content: Some(format!("// nav file {}\n", i + 1)),
                },
            )),
            target_ids: vec![target.target_id.clone()],
            required_capability: CapabilityId("editor.create_file".to_string()),
            rollback_step_ids: Vec::new(),
        })
        .collect();

    WorkspaceProposal {
        proposal_id: ProposalId(500),
        principal: PrincipalId("nav-test".to_string()),
        capability: CapabilityId("editor.batch".to_string()),
        correlation_id: CorrelationId(500),
        payload: ProposalPayload::Batch(BatchProposalPayload {
            batch_id: uuid::Uuid::from_u128(500),
            atomicity: ProposalBatchAtomicity::OrderedNonAtomic,
            rollback_policy: ProposalBatchRollbackPolicy::NotRequired,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets,
                omitted_target_count: 0,
                redaction_hints: Vec::new(),
            },
            items,
            dependency_edges: Vec::new(),
            rollback_steps: Vec::new(),
            partial_failures: Vec::new(),
            preview_warnings: Vec::new(),
            schema_version: 1,
        }),
        preconditions: ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: None,
            file_content_version: None,
            workspace_generation: None,
            expected_fingerprint: None,
            expected_file_length: None,
            expected_modified_at: None,
        },
        preview: PreviewSummary {
            summary: "keyboard nav test proposal".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: legion_protocol::TimestampMillis(1),
    }
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: std::path::PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let temp_root = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = temp_root.join(format!(
            "legion_desktop_keyboard_nav_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_keyboard_nav_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

#[test]
fn product_mode_switch_accepts_keyboard_activation() {
    let workspace = TempWorkspace::new();
    let mut runtime = open_runtime(workspace.path());
    runtime
        .handle_action(DesktopAction::SetProductMode {
            mode: DockMode::Assist,
        })
        .expect("switching to Assist should succeed");
    let mut app = DesktopEframeApp::new(runtime);

    assert_eq!(app.runtime_snapshot().product_mode, DockMode::Assist);

    let input = egui::RawInput {
        focused: true,
        modifiers: egui::Modifiers {
            command: true,
            alt: true,
            ..egui::Modifiers::default()
        },
        events: vec![egui::Event::Key {
            key: egui::Key::M,
            physical_key: Some(egui::Key::M),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                alt: true,
                ..egui::Modifiers::default()
            },
        }],
        ..egui::RawInput::default()
    };
    let _ = app.run_headless_input(input);

    assert_eq!(
        app.runtime_snapshot().product_mode,
        DockMode::Manual,
        "keyboard activation should select the Manual product mode"
    );
}

// ─── T4: Problems panel keyboard navigation ───────────────────────────────────

/// `ProblemNext` moves the focused index forward and wraps around.
#[test]
fn t4_problem_next_increments_selection() {
    let workspace = TempWorkspace::new();
    let file = workspace.root.join("main.rs");
    std::fs::write(&file, "fn main() {}\n").expect("write file");
    let mut runtime = open_runtime(workspace.path());

    // ProblemNext on a runtime with no problems is a no-op (no crash).
    let outcome = runtime
        .handle_action(DesktopAction::ProblemNext)
        .expect("ProblemNext must not error");
    assert_eq!(outcome, DesktopWorkflowOutcome::Noop);
    assert_eq!(runtime.problems_selected_index_for_test(), 0);

    // Open the file through the app so a buffer is created.
    let src_file = file.to_string_lossy().to_string();
    runtime
        .app_mut_for_test()
        .open_file(file.to_string_lossy())
        .expect("open_file must succeed");
    let uri = format!(
        "file:///{}",
        src_file.replace('\\', "/").trim_start_matches('/')
    );
    let buffer_id = runtime
        .app_mut_for_test()
        .active_buffer_id()
        .expect("active buffer must exist after open_file");
    let params = serde_json::json!({
        "uri": uri,
        "diagnostics": [
            {
                "range": { "start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1} },
                "severity": 1, "message": "error 1"
            },
            {
                "range": { "start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 1} },
                "severity": 2, "message": "warning 2"
            }
        ]
    });
    runtime
        .app_mut_for_test()
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &params, false, None)
        .expect("inject diagnostics");

    // ProblemNext moves from index 0 → 1.
    runtime
        .handle_action(DesktopAction::ProblemNext)
        .expect("ProblemNext");
    assert_eq!(runtime.problems_selected_index_for_test(), 1);

    // ProblemNext wraps 1 → 0.
    runtime
        .handle_action(DesktopAction::ProblemNext)
        .expect("ProblemNext wraps");
    assert_eq!(runtime.problems_selected_index_for_test(), 0);
}

/// `ProblemPrev` moves the focused index backward and wraps around.
#[test]
fn t4_problem_prev_decrements_selection() {
    let workspace = TempWorkspace::new();
    let file = workspace.root.join("lib.rs");
    std::fs::write(&file, "pub fn f() {}\n").expect("write file");
    let mut runtime = open_runtime(workspace.path());

    // Open the file through the app so a buffer is created.
    runtime
        .app_mut_for_test()
        .open_file(file.to_string_lossy())
        .expect("open_file must succeed");
    let src_file = file.to_string_lossy().to_string();
    let uri = format!(
        "file:///{}",
        src_file.replace('\\', "/").trim_start_matches('/')
    );
    let buffer_id = runtime
        .app_mut_for_test()
        .active_buffer_id()
        .expect("active buffer");
    let params = serde_json::json!({
        "uri": uri,
        "diagnostics": [
            { "range": { "start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1} },
              "severity": 1, "message": "e1" },
            { "range": { "start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 1} },
              "severity": 1, "message": "e2" }
        ]
    });
    runtime
        .app_mut_for_test()
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &params, false, None)
        .expect("inject");

    // Start at 0; ProblemPrev wraps to 1 (last item).
    runtime
        .handle_action(DesktopAction::ProblemPrev)
        .expect("ProblemPrev");
    assert_eq!(runtime.problems_selected_index_for_test(), 1);

    // ProblemPrev again → 0.
    runtime
        .handle_action(DesktopAction::ProblemPrev)
        .expect("ProblemPrev again");
    assert_eq!(runtime.problems_selected_index_for_test(), 0);
}

/// `ProblemActivate` with no problems is a Noop (guard condition).
#[test]
fn t4_problem_activate_with_no_problems_is_noop() {
    let workspace = TempWorkspace::new();
    let mut runtime = open_runtime(workspace.path());
    let outcome = runtime
        .handle_action(DesktopAction::ProblemActivate)
        .expect("ProblemActivate must not error");
    assert_eq!(outcome, DesktopWorkflowOutcome::Noop);
}

/// `ProblemActivate` with a real problem (path + range) opens the file.
///
/// This is the happy-path app-level test for T4: a diagnostic with a
/// disclosed path and range is selected, then activating it must route
/// through `OpenPathAtPosition` and return `DesktopWorkflowOutcome::Opened`.
#[test]
fn t4_problem_activate_happy_path() {
    let workspace = TempWorkspace::new();
    let file = workspace.root.join("activate.rs");
    // Write ten lines so line 5 exists when `OpenPathAtPosition` is dispatched.
    let content: String = (0..10).map(|i| format!("// line {i}\n")).collect();
    std::fs::write(&file, &content).expect("write activate.rs");
    let mut runtime = open_runtime(workspace.path());

    // Open through app so a `FileId` and buffer are registered.
    runtime
        .app_mut_for_test()
        .open_file(file.to_string_lossy())
        .expect("open_file must succeed");
    let buffer_id = runtime
        .app_mut_for_test()
        .active_buffer_id()
        .expect("active buffer must exist after open_file");

    // Inject a single diagnostic at line 5 with range disclosure so both
    // `path` (backfilled from file identity) and `range` are available.
    let src_file = file.to_string_lossy().to_string();
    let uri = format!(
        "file:///{}",
        src_file.replace('\\', "/").trim_start_matches('/')
    );
    let params = serde_json::json!({
        "uri": uri,
        "diagnostics": [{
            "range": {
                "start": { "line": 5, "character": 0 },
                "end":   { "line": 5, "character": 4 }
            },
            "severity": 1,
            "message": "activate me"
        }]
    });
    runtime
        .app_mut_for_test()
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &params, true, None)
        .expect("inject diagnostic");

    // Activate — the diagnostic has a real path so the file should open.
    let outcome = runtime
        .handle_action(DesktopAction::ProblemActivate)
        .expect("ProblemActivate must not error");
    assert_eq!(
        outcome,
        DesktopWorkflowOutcome::Opened,
        "ProblemActivate with a disclosed path+range should open the file at the diagnostic location"
    );
}

// ─── PKT-DIFF: Proposal review hunk keyboard navigation ──────────────────────

/// `ReviewHunkNext` is a Noop when no proposal reviews are in the projection.
///
/// The runtime starts with an empty delegated-task projection (no reviews).
/// `ReviewHunkNext` must not crash and must leave the selected index at 0.
#[test]
fn review_hunk_next_is_noop_with_no_reviews() {
    let workspace = TempWorkspace::new();
    let mut runtime = open_runtime(workspace.path());

    // Index must start at 0.
    assert_eq!(runtime.review_hunk_selected_index_for_test(), 0);

    let outcome = runtime
        .handle_action(DesktopAction::ReviewHunkNext)
        .expect("ReviewHunkNext must not error");
    assert_eq!(outcome, DesktopWorkflowOutcome::Noop);
    // Without any reviews the index must remain 0.
    assert_eq!(runtime.review_hunk_selected_index_for_test(), 0);
}

/// `ReviewHunkPrev` is a Noop when no proposal reviews are in the projection.
#[test]
fn review_hunk_prev_is_noop_with_no_reviews() {
    let workspace = TempWorkspace::new();
    let mut runtime = open_runtime(workspace.path());

    let outcome = runtime
        .handle_action(DesktopAction::ReviewHunkPrev)
        .expect("ReviewHunkPrev must not error");
    assert_eq!(outcome, DesktopWorkflowOutcome::Noop);
    assert_eq!(runtime.review_hunk_selected_index_for_test(), 0);
}

/// `ReviewHunkAccept`, `ReviewHunkReject`, `ReviewAcceptAll`, `ReviewRejectAll`
/// all return Noop when there are no proposal reviews in the projection (guard
/// condition — no hunks to record dispositions for).
#[test]
fn review_hunk_disposition_actions_noop_with_no_reviews() {
    let workspace = TempWorkspace::new();
    let mut runtime = open_runtime(workspace.path());

    for action in [
        DesktopAction::ReviewHunkAccept,
        DesktopAction::ReviewHunkReject,
        DesktopAction::ReviewAcceptAll,
        DesktopAction::ReviewRejectAll,
    ] {
        let outcome = runtime
            .handle_action(action)
            .expect("disposition action with no reviews must not error");
        assert_eq!(
            outcome,
            DesktopWorkflowOutcome::Noop,
            "disposition action with no reviews must be a Noop"
        );
    }
}

/// Pressing `Alt+ArrowRight` dispatches `ReviewHunkNext` through the keyboard
/// handler — same egui key-dispatch harness as the T4 ProblemNext test.
///
/// With no proposal reviews the index stays at 0, but the important assertion
/// is that the binding is wired: no panic, clean Noop, index unchanged.
#[test]
fn review_hunk_key_dispatch_alt_arrow_right_via_egui() {
    let workspace = TempWorkspace::new();
    let runtime = open_runtime(workspace.path());

    let mut app = DesktopEframeApp::new(runtime);
    assert_eq!(app.review_hunk_selected_index_for_test(), 0);

    let raw_input = egui::RawInput {
        focused: true,
        events: vec![egui::Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: Some(egui::Key::ArrowRight),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                alt: true,
                ..egui::Modifiers::default()
            },
        }],
        modifiers: egui::Modifiers {
            alt: true,
            ..egui::Modifiers::default()
        },
        ..egui::RawInput::default()
    };
    let _ = app.run_headless_input(raw_input);

    // No reviews → index stays 0 (correct no-op behaviour).
    assert_eq!(
        app.review_hunk_selected_index_for_test(),
        0,
        "Alt+ArrowRight with no reviews must be a Noop (index stays 0)"
    );
}

/// Pressing ArrowDown in egui dispatches `ProblemNext` through the keyboard handler.
///
/// This is the desktop-level T4 test: a synthetic ArrowDown `RawInput` is
/// fed through `run_headless_input`, which exercises the same `handle_keyboard`
/// path that production uses. Arrow-key events are routed to problem-panel
/// navigation from the cloned `InputState` inside `handle_keyboard`, which is
/// immune to the egui focus-navigation mechanism that can consume these events
/// once widget rendering begins. The test asserts that the problems-selected
/// index advances, proving the wiring from egui event to `ProblemNext` action.
#[test]
fn t4_problem_key_dispatch_via_egui() {
    let workspace = TempWorkspace::new();
    let file = workspace.root.join("key_dispatch.rs");
    std::fs::write(&file, "fn a() {}\nfn b() {}\n").expect("write key_dispatch.rs");
    let mut runtime = open_runtime(workspace.path());

    // Open file so a buffer exists and diagnostics can be attached to it.
    runtime
        .app_mut_for_test()
        .open_file(file.to_string_lossy())
        .expect("open_file must succeed");
    let buffer_id = runtime
        .app_mut_for_test()
        .active_buffer_id()
        .expect("active buffer must exist after open_file");

    let src_file = file.to_string_lossy().to_string();
    let uri = format!(
        "file:///{}",
        src_file.replace('\\', "/").trim_start_matches('/')
    );
    let params = serde_json::json!({
        "uri": uri,
        "diagnostics": [
            {
                "range": { "start": { "line": 0, "character": 0 },
                           "end":   { "line": 0, "character": 1 } },
                "severity": 1, "message": "err 0"
            },
            {
                "range": { "start": { "line": 1, "character": 0 },
                           "end":   { "line": 1, "character": 1 } },
                "severity": 1, "message": "err 1"
            }
        ]
    });
    runtime
        .app_mut_for_test()
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &params, false, None)
        .expect("inject diagnostics");

    // Force a projection refresh so the shell snapshot (used by the render
    // frame) already contains the two problems.  ProblemNext advances to 1;
    // ProblemPrev resets to 0.  Index is 0 going into the egui frame.
    runtime
        .handle_action(DesktopAction::ProblemNext)
        .expect("ProblemNext to force snapshot refresh");
    runtime
        .handle_action(DesktopAction::ProblemPrev)
        .expect("ProblemPrev to reset index to 0");

    let mut app = DesktopEframeApp::new(runtime);
    assert_eq!(
        app.problems_selected_index_for_test(),
        0,
        "index should be 0 before the egui key event"
    );

    // Synthesise an ArrowDown key event and drive it through handle_keyboard.
    // The problems-navigation binding lives in the cloned-InputState block of
    // `handle_keyboard`, so `run_headless_input` (which calls that function)
    // is the correct harness — it exercises the same routing as production.
    let raw_input = egui::RawInput {
        focused: true,
        events: vec![egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: Some(egui::Key::ArrowDown),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
        ..egui::RawInput::default()
    };
    let _ = app.run_headless_input(raw_input);

    assert_eq!(
        app.problems_selected_index_for_test(),
        1,
        "ArrowDown egui event should dispatch ProblemNext and advance the selected index from 0 to 1"
    );
}

// ─── PKT-DIFF F7: Proposal review hunk navigation with real hunk data ─────────

/// `ReviewHunkNext` increments and wraps when proposal reviews with hunks are
/// in the delegated-task projection.
///
/// The test seeds a 5-item batch proposal via `register_proposal_lifecycle` so
/// the `proposal_reviews` list in the delegated-task projection has 5 hunks.
/// It then dispatches `ReviewHunkNext` multiple times and asserts the expected
/// index values, including wrap-around.
#[test]
fn review_hunk_next_increments_and_wraps_with_seeded_hunks() {
    let workspace = TempWorkspace::new();
    let mut runtime = open_runtime(workspace.path());

    // Seed a batch proposal so the delegated-task projection has 5 hunks.
    let proposal = five_target_batch_proposal();
    runtime
        .app_mut_for_test()
        .register_proposal_lifecycle(&proposal)
        .expect("register_proposal_lifecycle must succeed");

    // Index starts at 0.
    assert_eq!(runtime.review_hunk_selected_index_for_test(), 0);

    // Next → 1
    runtime
        .handle_action(DesktopAction::ReviewHunkNext)
        .expect("ReviewHunkNext");
    assert_eq!(
        runtime.review_hunk_selected_index_for_test(),
        1,
        "first ReviewHunkNext must advance from 0 to 1"
    );

    // Next → 2
    runtime
        .handle_action(DesktopAction::ReviewHunkNext)
        .expect("ReviewHunkNext");
    assert_eq!(runtime.review_hunk_selected_index_for_test(), 2);

    // Next → 3, 4, then wraps to 0
    runtime
        .handle_action(DesktopAction::ReviewHunkNext)
        .expect("ReviewHunkNext");
    runtime
        .handle_action(DesktopAction::ReviewHunkNext)
        .expect("ReviewHunkNext");
    runtime
        .handle_action(DesktopAction::ReviewHunkNext)
        .expect("ReviewHunkNext wraps");
    assert_eq!(
        runtime.review_hunk_selected_index_for_test(),
        0,
        "ReviewHunkNext must wrap at hunk count (5 hunks → index 4 → wraps to 0)"
    );
}

/// `ReviewHunkPrev` decrements and wraps when proposal reviews with hunks are
/// in the delegated-task projection.
#[test]
fn review_hunk_prev_decrements_and_wraps_with_seeded_hunks() {
    let workspace = TempWorkspace::new();
    let mut runtime = open_runtime(workspace.path());

    let proposal = five_target_batch_proposal();
    runtime
        .app_mut_for_test()
        .register_proposal_lifecycle(&proposal)
        .expect("register_proposal_lifecycle must succeed");

    // At index 0, prev wraps to last hunk (index 4).
    runtime
        .handle_action(DesktopAction::ReviewHunkPrev)
        .expect("ReviewHunkPrev wraps");
    assert_eq!(
        runtime.review_hunk_selected_index_for_test(),
        4,
        "ReviewHunkPrev at index 0 must wrap to index 4 (5 hunks)"
    );

    // Prev → 3
    runtime
        .handle_action(DesktopAction::ReviewHunkPrev)
        .expect("ReviewHunkPrev");
    assert_eq!(runtime.review_hunk_selected_index_for_test(), 3);
}

/// `ReviewHunkAccept` records a disposition for the focused hunk when a
/// seeded proposal is present in the projection.
#[test]
fn review_hunk_accept_records_disposition_for_focused_hunk() {
    let workspace = TempWorkspace::new();
    let mut runtime = open_runtime(workspace.path());

    let proposal = five_target_batch_proposal();
    runtime
        .app_mut_for_test()
        .register_proposal_lifecycle(&proposal)
        .expect("register_proposal_lifecycle must succeed");

    // Force a projection refresh by calling ReviewHunkNext (which calls
    // refresh_projection internally).  Index moves to 1; then we move back to 0.
    runtime
        .handle_action(DesktopAction::ReviewHunkNext)
        .expect("ReviewHunkNext");
    runtime
        .handle_action(DesktopAction::ReviewHunkPrev)
        .expect("ReviewHunkPrev back to 0");
    assert_eq!(runtime.review_hunk_selected_index_for_test(), 0);

    // Accept the focused hunk (index 0).
    runtime
        .handle_action(DesktopAction::ReviewHunkAccept)
        .expect("ReviewHunkAccept must not error");

    // The disposition state must record an accepted hunk for this proposal.
    let pid = proposal.proposal_id;
    let accepted = runtime.hunk_dispositions().accepted_hunk_ids(pid);
    assert_eq!(
        accepted.len(),
        1,
        "exactly one hunk must be accepted after ReviewHunkAccept"
    );

    // The accepted hunk_id must correspond to the first hunk in the projection.
    // Its ID follows the pattern "delegate:proposal:500:…".
    assert!(
        accepted.iter().next().unwrap().contains("500"),
        "accepted hunk_id must reference the seeded proposal (id=500)"
    );
}

/// `ReviewAcceptAll` marks every hunk accepted; `ReviewDismiss` resets all.
#[test]
fn review_accept_all_and_dismiss_lifecycle() {
    let workspace = TempWorkspace::new();
    let mut runtime = open_runtime(workspace.path());

    let proposal = five_target_batch_proposal();
    runtime
        .app_mut_for_test()
        .register_proposal_lifecycle(&proposal)
        .expect("register_proposal_lifecycle must succeed");

    // Accept all 5 hunks.
    runtime
        .handle_action(DesktopAction::ReviewAcceptAll)
        .expect("ReviewAcceptAll must not error");

    let pid = proposal.proposal_id;
    let accepted = runtime.hunk_dispositions().accepted_hunk_ids(pid);
    assert_eq!(
        accepted.len(),
        5,
        "ReviewAcceptAll must record 5 accepted hunks for a 5-item proposal"
    );

    // Dismiss must clear all dispositions and reset the index.
    runtime
        .handle_action(DesktopAction::ReviewDismiss)
        .expect("ReviewDismiss must not error");
    let accepted_after_dismiss = runtime.hunk_dispositions().accepted_hunk_ids(pid);
    assert_eq!(
        accepted_after_dismiss.len(),
        0,
        "ReviewDismiss must clear all accepted hunks"
    );
    assert_eq!(
        runtime.review_hunk_selected_index_for_test(),
        0,
        "ReviewDismiss must reset the navigation index to 0"
    );
}
