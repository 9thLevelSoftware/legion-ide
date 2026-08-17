//! Multi-file search-and-replace must arrive as a reviewable proposal.
//!
//! The load-bearing property under test is the authority boundary: a replace
//! that spans several files produces one proposal that covers *every* file it
//! would touch, writes to *none* of them until it is approved and applied, and
//! — once applied — can be reversed in every file, not only the active one.
//!
//! Applying a text-edit proposal writes to the editor buffer, not to disk;
//! saving is a separate step. The declared reversal for a text-edit route is
//! therefore editor undo (`ProposalRollbackAction::EditorUndoGroup`), which is
//! what these tests exercise.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use legion_app::{AppComposition, SearchQueryOptions};
use legion_editor::{TextEdit, TextPosition};
use legion_protocol::{
    BufferId, PrincipalId, ProposalPayload, ProposalRejectionReason, ProposalRequest,
    ProposalResponse, ProposalTargetCoverageKind, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "legion_workspace_replace_{}_{}_{}",
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

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, content).expect("temp file should be written");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_workspace_replace_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

/// A file seeded on disk together with the buffer it was opened into.
struct OpenedFile {
    path: PathBuf,
    buffer_id: BufferId,
    original: String,
}

/// Seed three files that all contain `alpha`, open each one, and return them in
/// seed order. The last-opened file is the active buffer, so files 0 and 1 are
/// the "not active" cases the reversal test depends on.
fn workspace_with_three_open_matches(
    workspace: &TempWorkspace,
    app: &mut AppComposition,
) -> Vec<OpenedFile> {
    let seeds = [
        ("one.txt", "alpha one\nalpha again\n"),
        ("two.txt", "beta\nalpha two\n"),
        ("three.txt", "alpha three\n"),
    ];
    let opened_workspace = app
        .open_workspace(
            workspace.path(),
            WorkspaceTrustState::Trusted,
            PrincipalId("workspace-replace-test".to_string()),
        )
        .expect("workspace should open");

    seeds
        .iter()
        .map(|(name, content)| {
            let path = workspace.write(name, content);
            let file_id = app.open_file(path.to_string_lossy()).expect("open file");
            let buffer_id = app
                .editor()
                .buffer_for_file(opened_workspace.workspace_id, file_id)
                .expect("buffer for opened file");
            OpenedFile {
                path,
                buffer_id,
                original: (*content).to_string(),
            }
        })
        .collect()
}

fn buffer_text(app: &AppComposition, buffer_id: BufferId) -> String {
    app.editor()
        .text(buffer_id)
        .expect("buffer text")
        .to_string()
}

fn disk_text(path: &Path) -> String {
    fs::read_to_string(path).expect("read file from disk")
}

/// Approve then apply a proposal through the product lifecycle path.
fn approve_and_apply(
    app: &mut AppComposition,
    proposal_id: legion_protocol::ProposalId,
) -> ProposalResponse {
    match app
        .dispatch_ui_intent(CommandDispatchIntent::ApproveProposal { proposal_id })
        .expect("approve proposal")
    {
        legion_app::AppCommandOutcome::ProposalLifecycleUpdated(ProposalResponse::Approved(_)) => {}
        other => panic!("expected approval, got {other:?}"),
    }
    match app
        .dispatch_ui_intent(CommandDispatchIntent::ApplyProposal { proposal_id })
        .expect("apply proposal")
    {
        legion_app::AppCommandOutcome::ProposalLifecycleUpdated(response) => response,
        other => panic!("expected lifecycle response, got {other:?}"),
    }
}

#[test]
fn workspace_replace_proposes_every_matching_file_and_writes_none() {
    let workspace = TempWorkspace::new();
    let mut app = AppComposition::new();
    let files = workspace_with_three_open_matches(&workspace, &mut app);

    let outcome = app
        .propose_workspace_replace("alpha", "omega", SearchQueryOptions::default(), 0)
        .expect("replace proposal should build");

    let proposal_id = outcome
        .proposal_id
        .expect("multi-file replace must produce a proposal");
    assert_eq!(
        outcome.edited_file_count, 3,
        "every matching file must be covered: {:?}",
        outcome.diagnostics
    );
    assert_eq!(
        outcome.edit_count, 4,
        "four `alpha` occurrences across the three files"
    );
    assert!(
        outcome.skipped_closed_files.is_empty(),
        "all three files are open"
    );

    let proposal = app
        .workspace_proposal_for_id(proposal_id)
        .expect("proposal should be retrievable for review");
    let ProposalPayload::WorkspaceEdit(payload) = &proposal.payload else {
        panic!("replace must be a workspace-edit proposal, got {proposal:?}");
    };
    assert_eq!(payload.file_edits.len(), 3);
    assert_eq!(
        payload.target_coverage.coverage_kind,
        ProposalTargetCoverageKind::Complete,
        "no matching file was omitted"
    );
    assert_eq!(payload.target_coverage.targets.len(), 3);
    assert_eq!(payload.target_coverage.omitted_target_count, 0);
    for file in &files {
        assert!(
            payload
                .target_coverage
                .targets
                .iter()
                .any(|target| target.buffer_id == Some(file.buffer_id)),
            "coverage is missing buffer {:?}",
            file.buffer_id
        );
    }

    // The authority boundary: proposing changed nothing anywhere.
    for file in &files {
        assert_eq!(
            buffer_text(&app, file.buffer_id),
            file.original,
            "buffer for {} was mutated by proposing",
            file.path.display()
        );
        assert_eq!(
            disk_text(&file.path),
            file.original,
            "disk file {} was mutated by proposing",
            file.path.display()
        );
    }
}

#[test]
fn workspace_replace_apply_edits_every_buffer_and_leaves_disk_untouched() {
    let workspace = TempWorkspace::new();
    let mut app = AppComposition::new();
    let files = workspace_with_three_open_matches(&workspace, &mut app);

    let proposal_id = app
        .propose_workspace_replace("alpha", "omega", SearchQueryOptions::default(), 0)
        .expect("replace proposal should build")
        .proposal_id
        .expect("proposal id");

    let response = approve_and_apply(&mut app, proposal_id);
    assert!(
        matches!(response, ProposalResponse::Applied(_)),
        "apply response: {response:?}"
    );

    let expected = [
        "omega one\nomega again\n",
        "beta\nomega two\n",
        "omega three\n",
    ];
    for (file, expected) in files.iter().zip(expected) {
        assert_eq!(
            buffer_text(&app, file.buffer_id),
            expected,
            "buffer for {} was not replaced",
            file.path.display()
        );
        // Applying a text-edit proposal writes to the buffer, never to disk;
        // saving is a separate, separately-authorised step.
        assert_eq!(
            disk_text(&file.path),
            file.original,
            "disk file {} must be untouched by a buffer apply",
            file.path.display()
        );
    }
}

#[test]
fn workspace_replace_undo_reverts_every_buffer_not_only_the_active_one() {
    let workspace = TempWorkspace::new();
    let mut app = AppComposition::new();
    let files = workspace_with_three_open_matches(&workspace, &mut app);

    let proposal_id = app
        .propose_workspace_replace("alpha", "omega", SearchQueryOptions::default(), 0)
        .expect("replace proposal should build")
        .proposal_id
        .expect("proposal id");
    assert!(matches!(
        approve_and_apply(&mut app, proposal_id),
        ProposalResponse::Applied(_)
    ));

    // The declared reversal for a text-edit route is editor undo. Each file's
    // replacements were applied as one undo group, so exactly one undo per
    // buffer must restore it — including the two buffers that are not active.
    for file in &files {
        app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
            buffer_id: file.buffer_id,
        })
        .expect("switch to buffer before undo");
        app.dispatch_ui_intent(CommandDispatchIntent::Undo {
            buffer_id: file.buffer_id,
        })
        .expect("undo buffer");
    }

    for file in &files {
        assert_eq!(
            buffer_text(&app, file.buffer_id),
            file.original,
            "undo did not reach {} — a half-reversed replace is worse than none",
            file.path.display()
        );
    }
}

#[test]
fn workspace_replace_apply_mutates_nothing_when_one_target_went_stale() {
    let workspace = TempWorkspace::new();
    let mut app = AppComposition::new();
    let files = workspace_with_three_open_matches(&workspace, &mut app);

    let proposal_id = app
        .propose_workspace_replace("alpha", "omega", SearchQueryOptions::default(), 0)
        .expect("replace proposal should build")
        .proposal_id
        .expect("proposal id");

    // Invalidate the middle target only. The whole proposal must fail closed:
    // the apply path preflights every file edit before mutating any of them, so
    // the two still-valid buffers must not be edited either.
    let stale = &files[1];
    app.dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
        buffer_id: stale.buffer_id,
    })
    .expect("switch to the target being invalidated");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 0), "drift "))
        .expect("edit buffer after the proposal was built");

    let response = approve_and_apply(&mut app, proposal_id);
    assert!(
        !matches!(response, ProposalResponse::Applied(_)),
        "a stale target must not apply: {response:?}"
    );

    assert_eq!(
        buffer_text(&app, files[0].buffer_id),
        files[0].original,
        "an unrelated buffer was mutated by a failed apply"
    );
    assert_eq!(
        buffer_text(&app, files[2].buffer_id),
        files[2].original,
        "an unrelated buffer was mutated by a failed apply"
    );
    assert_eq!(
        buffer_text(&app, stale.buffer_id),
        format!("drift {}", stale.original),
        "the stale buffer should still hold only the manual edit"
    );
    for file in &files {
        assert_eq!(disk_text(&file.path), file.original);
    }
}

#[test]
fn workspace_replace_refuses_when_the_match_set_is_truncated() {
    let workspace = TempWorkspace::new();
    let mut app = AppComposition::new();
    let files = workspace_with_three_open_matches(&workspace, &mut app);

    // Four matches exist; a two-result bound cannot see them all. Replacing a
    // prefix of the matches would look like a completed replace, so the
    // proposal is withheld entirely.
    let outcome = app
        .propose_workspace_replace("alpha", "omega", SearchQueryOptions::default(), 2)
        .expect("replace call should not error");

    assert!(
        outcome.proposal_id.is_none(),
        "a truncated match set must not produce a proposal"
    );
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("Replace refused")),
        "diagnostics should explain the refusal: {:?}",
        outcome.diagnostics
    );
    for file in &files {
        assert_eq!(buffer_text(&app, file.buffer_id), file.original);
        assert_eq!(disk_text(&file.path), file.original);
    }
}

#[test]
fn workspace_replace_refuses_when_a_matching_file_is_not_open() {
    let workspace = TempWorkspace::new();
    let mut app = AppComposition::new();
    let opened_workspace = app
        .open_workspace(
            workspace.path(),
            WorkspaceTrustState::Trusted,
            PrincipalId("workspace-replace-test".to_string()),
        )
        .expect("workspace should open");

    let open_path = workspace.write("open.txt", "alpha open\n");
    let closed_path = workspace.write("closed.txt", "alpha closed\n");
    let open_file_id = app
        .open_file(open_path.to_string_lossy())
        .expect("open the first file only");
    let open_buffer = app
        .editor()
        .buffer_for_file(opened_workspace.workspace_id, open_file_id)
        .expect("buffer for opened file");

    let outcome = app
        .propose_workspace_replace("alpha", "omega", SearchQueryOptions::default(), 0)
        .expect("replace proposal should build");

    // A workspace edit may not carry partial target coverage — the protocol
    // validator denies it with `proposal.incomplete_target_coverage`. So a
    // replace whose match set reaches a closed file has to decline as a whole
    // rather than quietly narrow itself to the files it can reach.
    assert!(
        outcome.proposal_id.is_none(),
        "an unreachable match must not yield a partially-covering proposal"
    );
    assert_eq!(
        outcome
            .skipped_closed_files
            .iter()
            .filter(|path| path.0.ends_with("closed.txt"))
            .count(),
        1,
        "the closed match must be named, not silently dropped: {:?}",
        outcome.skipped_closed_files
    );
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("Replace refused")),
        "diagnostics should explain the refusal: {:?}",
        outcome.diagnostics
    );

    assert_eq!(buffer_text(&app, open_buffer), "alpha open\n");
    assert_eq!(disk_text(&open_path), "alpha open\n");
    assert_eq!(
        disk_text(&closed_path),
        "alpha closed\n",
        "the closed file must never be written by a replace"
    );
}

#[test]
fn workspace_replace_proposal_is_registered_for_review_before_apply() {
    let workspace = TempWorkspace::new();
    let mut app = AppComposition::new();
    let files = workspace_with_three_open_matches(&workspace, &mut app);

    let proposal_id = app
        .propose_workspace_replace("alpha", "omega", SearchQueryOptions::default(), 0)
        .expect("replace proposal should build")
        .proposal_id
        .expect("proposal id");

    // Rejecting the proposal is a terminal review decision: the subsequent apply
    // must be refused and every buffer must still hold its original text.
    let rejected = app
        .dispatch_ui_intent(CommandDispatchIntent::RejectProposal {
            proposal_id,
            reason: ProposalRejectionReason::UserRejected,
        })
        .expect("reject proposal");
    assert!(
        matches!(
            rejected,
            legion_app::AppCommandOutcome::ProposalLifecycleUpdated(
                ProposalResponse::Rejected { .. }
            )
        ),
        "reject response: {rejected:?}"
    );

    let proposal = app
        .workspace_proposal_for_id(proposal_id)
        .expect("proposal should still be retrievable after rejection");
    let applied = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply after rejection");
    assert!(
        !matches!(applied, ProposalResponse::Applied(_)),
        "a rejected replace must not apply: {applied:?}"
    );
    for file in &files {
        assert_eq!(buffer_text(&app, file.buffer_id), file.original);
        assert_eq!(disk_text(&file.path), file.original);
    }
}
