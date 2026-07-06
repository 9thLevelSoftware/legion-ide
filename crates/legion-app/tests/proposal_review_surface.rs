//! Integration tests for PKT-DIFF — multi-file proposal review surface.
//!
//! # Coverage
//!
//! T2 (P3.F2.T1): 5-file batch proposal → diff surface with 5 sections; partial
//!                acceptance via `filtered_batch_proposal_for_accepted_targets`.
//! T3 (P3.F2.T2): Per-hunk accept/reject with undo support.
//! T4 (P3.F2.T4): Evidence panel DTO fields are structured (no free-text pass-through).

use std::collections::{HashMap, HashSet};

use legion_app::proposal::{
    ProposalHunkDispositionState, compute_proposal_diff_surface,
    filtered_batch_proposal_for_accepted_hunks, filtered_batch_proposal_for_accepted_targets,
};
use legion_protocol::{
    BatchProposalPayload, CanonicalPath, CapabilityId, CorrelationId,
    DelegatedTaskProposalHunkDisposition, FileId, PreviewSummary, PrincipalId,
    ProposalAffectedTarget, ProposalBatchAtomicity, ProposalBatchItem, ProposalBatchRollbackPolicy,
    ProposalId, ProposalPayload, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProposalTargetKind, ProposalVersionPreconditions, TimestampMillis, WorkspaceId,
    WorkspaceProposal,
};
use uuid::Uuid;

// ─── Fixtures ────────────────────────────────────────────────────────────────

fn five_file_batch_proposal() -> WorkspaceProposal {
    let file_ids: Vec<FileId> = (1..=5).map(FileId).collect();
    let target_ids: Vec<String> = (1..=5).map(|i| format!("target-file-{i}")).collect();

    let targets: Vec<ProposalAffectedTarget> = file_ids
        .iter()
        .zip(target_ids.iter())
        .enumerate()
        .map(|(i, (fid, tid))| ProposalAffectedTarget {
            target_id: tid.clone(),
            kind: ProposalTargetKind::ClosedFile,
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(*fid),
            buffer_id: None,
            path: Some(CanonicalPath(format!("src/file_{}.rs", i + 1))),
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
            item_id: format!("item-{}", i + 1),
            payload: Box::new(ProposalPayload::CreateFile(
                legion_protocol::CreateFileProposal {
                    path: target.path.clone().unwrap(),
                    initial_content: Some(format!("// file {}\n", i + 1)),
                },
            )),
            target_ids: vec![target.target_id.clone()],
            required_capability: CapabilityId("editor.create_file".to_string()),
            rollback_step_ids: Vec::new(),
        })
        .collect();

    WorkspaceProposal {
        proposal_id: ProposalId(100),
        principal: PrincipalId("test-principal".to_string()),
        capability: CapabilityId("editor.batch".to_string()),
        correlation_id: CorrelationId(1),
        payload: ProposalPayload::Batch(BatchProposalPayload {
            batch_id: Uuid::from_u128(999),
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
            summary: "5-file batch diff surface test".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    }
}

/// Build `(old_text, new_text)` pairs for each of the 5 targets. Target 1–3
/// have real text changes; targets 4–5 have identical before/after (no hunks).
fn five_file_contents() -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    map.insert(
        "target-file-1".to_string(),
        (
            "fn main() {}\n".to_string(),
            "fn main() {\n    println!(\"hello\");\n}\n".to_string(),
        ),
    );
    map.insert(
        "target-file-2".to_string(),
        (
            "let x = 1;\nlet y = 2;\n".to_string(),
            "let x = 10;\nlet y = 20;\n".to_string(),
        ),
    );
    map.insert(
        "target-file-3".to_string(),
        (
            "old line 1\nold line 2\n".to_string(),
            "new line A\nnew line B\n".to_string(),
        ),
    );
    map.insert(
        "target-file-4".to_string(),
        (
            "unchanged content\n".to_string(),
            "unchanged content\n".to_string(),
        ),
    );
    map.insert(
        "target-file-5".to_string(),
        (
            "also unchanged\n".to_string(),
            "also unchanged\n".to_string(),
        ),
    );
    map
}

// ─── Task 2 tests ─────────────────────────────────────────────────────────────

/// T2-A: 5-file batch proposal produces a diff surface with exactly 5 sections.
#[test]
fn five_file_proposal_produces_five_sections() {
    let proposal = five_file_batch_proposal();
    let contents = five_file_contents();
    let surface = compute_proposal_diff_surface(&proposal, &contents);
    assert_eq!(
        surface.sections.len(),
        5,
        "one section per target: expected 5, got {}",
        surface.sections.len()
    );
    // The active section should be the first one.
    assert!(
        surface.active_section_id.is_some(),
        "active_section_id must be set when sections are present"
    );
}

/// T2-B: Sections with real changes have at least one chunk with changed lines.
#[test]
fn changed_files_have_non_empty_chunks() {
    let proposal = five_file_batch_proposal();
    let contents = five_file_contents();
    let surface = compute_proposal_diff_surface(&proposal, &contents);

    let changed_target_ids = ["target-file-1", "target-file-2", "target-file-3"];
    for tid in &changed_target_ids {
        let section = surface
            .sections
            .iter()
            .find(|s| s.target_id.as_deref() == Some(tid))
            .unwrap_or_else(|| panic!("section for {tid} not found"));
        let total_changed: u32 = section.chunks.iter().map(|c| c.changed_line_count).sum();
        assert!(
            total_changed > 0,
            "section for {tid} should have at least one changed line"
        );
    }
}

/// T2-C: Accepting only 2 targets via `filtered_batch_proposal_for_accepted_targets`
/// produces a proposal with exactly those 2 targets' items.
#[test]
fn partial_accept_filters_to_two_targets() {
    let proposal = five_file_batch_proposal();
    let accepted_ids: HashSet<String> = ["target-file-1", "target-file-3"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let filtered = filtered_batch_proposal_for_accepted_targets(&proposal, &accepted_ids)
        .expect("filtered proposal must be produced for non-empty accepted set");

    let ProposalPayload::Batch(ref batch) = filtered.payload else {
        panic!("filtered proposal must be a Batch payload");
    };
    assert_eq!(
        batch.target_coverage.targets.len(),
        2,
        "filtered proposal must contain exactly 2 targets"
    );
    let retained_ids: HashSet<&str> = batch
        .target_coverage
        .targets
        .iter()
        .map(|t| t.target_id.as_str())
        .collect();
    assert!(retained_ids.contains("target-file-1"));
    assert!(retained_ids.contains("target-file-3"));
}

/// T2-D: `compute_proposal_diff_surface` on a non-batch proposal returns an
/// empty surface (graceful no-op).
#[test]
fn non_batch_proposal_produces_empty_surface() {
    let proposal = WorkspaceProposal {
        proposal_id: ProposalId(1),
        principal: PrincipalId("p".to_string()),
        capability: CapabilityId("c".to_string()),
        correlation_id: CorrelationId(1),
        payload: ProposalPayload::CreateFile(legion_protocol::CreateFileProposal {
            path: CanonicalPath("src/new.rs".to_string()),
            initial_content: Some("fn main() {}\n".to_string()),
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
            summary: "create file".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    };
    let surface = compute_proposal_diff_surface(&proposal, &HashMap::new());
    assert!(
        surface.sections.is_empty(),
        "non-batch proposal → empty diff surface"
    );
}

// ─── Task 3 tests ─────────────────────────────────────────────────────────────

/// T3-A: `ProposalHunkDispositionState` starts with Pending defaults and
/// correctly records Accept/Reject decisions.
#[test]
fn hunk_disposition_state_defaults_to_pending() {
    let mut state = ProposalHunkDispositionState::new();
    let pid = ProposalId(1);

    // Default is Pending.
    assert_eq!(
        state.disposition(pid, "hunk-1"),
        DelegatedTaskProposalHunkDisposition::Pending
    );

    // Accept one hunk.
    state.set_hunk_disposition(
        pid,
        "hunk-1",
        DelegatedTaskProposalHunkDisposition::Accepted,
    );
    assert_eq!(
        state.disposition(pid, "hunk-1"),
        DelegatedTaskProposalHunkDisposition::Accepted
    );

    // Reject another.
    state.set_hunk_disposition(
        pid,
        "hunk-2",
        DelegatedTaskProposalHunkDisposition::Rejected,
    );
    assert_eq!(
        state.disposition(pid, "hunk-2"),
        DelegatedTaskProposalHunkDisposition::Rejected
    );
}

/// T3-B: Undoing a disposition change restores the previous value.
#[test]
fn undo_disposition_change_restores_previous() {
    let mut state = ProposalHunkDispositionState::new();
    let pid = ProposalId(1);

    // Pending → Accept.
    state.set_hunk_disposition(
        pid,
        "hunk-1",
        DelegatedTaskProposalHunkDisposition::Accepted,
    );
    assert_eq!(
        state.disposition(pid, "hunk-1"),
        DelegatedTaskProposalHunkDisposition::Accepted
    );

    // Undo: should restore to Pending.
    let undone = state.undo_last_disposition_change();
    assert!(undone, "undo must return true when stack is non-empty");
    assert_eq!(
        state.disposition(pid, "hunk-1"),
        DelegatedTaskProposalHunkDisposition::Pending,
        "undo must restore Pending"
    );
}

/// T3-C: Multiple undo operations restore changes in LIFO order.
#[test]
fn multiple_undo_operations_are_lifo() {
    let mut state = ProposalHunkDispositionState::new();
    let pid = ProposalId(2);

    state.set_hunk_disposition(pid, "h1", DelegatedTaskProposalHunkDisposition::Accepted);
    state.set_hunk_disposition(pid, "h2", DelegatedTaskProposalHunkDisposition::Rejected);
    state.set_hunk_disposition(pid, "h1", DelegatedTaskProposalHunkDisposition::Rejected);

    // Undo most recent (h1 Rejected → Accepted).
    state.undo_last_disposition_change();
    assert_eq!(
        state.disposition(pid, "h1"),
        DelegatedTaskProposalHunkDisposition::Accepted
    );
    // h2 is unaffected.
    assert_eq!(
        state.disposition(pid, "h2"),
        DelegatedTaskProposalHunkDisposition::Rejected
    );

    // Undo again (h2 Rejected → Pending).
    state.undo_last_disposition_change();
    assert_eq!(
        state.disposition(pid, "h2"),
        DelegatedTaskProposalHunkDisposition::Pending
    );
}

/// T3-D: Undo on an empty stack returns false and leaves state unchanged.
#[test]
fn undo_on_empty_stack_returns_false() {
    let mut state = ProposalHunkDispositionState::new();
    assert!(!state.undo_last_disposition_change());
    assert_eq!(state.undo_depth(), 0);
}

/// T2-E (F2 conservative): accepting only SOME hunks of a target excludes the
/// entire target from the filtered result.
///
/// A file with two well-separated diff hunks is used; accepting only one of them
/// must NOT include that file — the conservative policy requires all hunks to be
/// accepted before a target proceeds to the apply path.
#[test]
fn partial_hunk_accept_excludes_whole_target_conservative() {
    use legion_protocol::{
        BatchProposalPayload, CanonicalPath, CapabilityId, CorrelationId, FileId, PreviewSummary,
        PrincipalId, ProposalAffectedTarget, ProposalBatchAtomicity, ProposalBatchItem,
        ProposalBatchRollbackPolicy, ProposalId, ProposalPayload, ProposalTargetCoverage,
        ProposalTargetCoverageKind, ProposalTargetKind, ProposalVersionPreconditions,
        TimestampMillis, WorkspaceId, WorkspaceProposal,
    };

    // Build old/new texts for one file where the changes are far enough apart
    // (>2*CONTEXT_LINES) to produce two separate diff hunks.
    let old_text = (0..16).map(|i| format!("line {i}\n")).collect::<String>();
    let new_text = {
        let mut lines: Vec<String> = (0..16).map(|i| format!("line {i}\n")).collect();
        lines[0] = "changed first\n".to_string();
        lines[14] = "changed last\n".to_string();
        lines.join("")
    };

    let target = ProposalAffectedTarget {
        target_id: "two-hunk-file".to_string(),
        kind: ProposalTargetKind::ClosedFile,
        workspace_id: Some(WorkspaceId(1)),
        file_id: Some(FileId(99)),
        buffer_id: None,
        path: Some(CanonicalPath("src/two_hunk.rs".to_string())),
        terminal_session_id: None,
        plugin_id: None,
        remote_authority: None,
        collaboration_session_id: None,
        byte_ranges: Vec::new(),
        redaction_hints: Vec::new(),
    };
    let item = ProposalBatchItem {
        order: 0,
        item_id: "two-hunk-item".to_string(),
        payload: Box::new(ProposalPayload::CreateFile(
            legion_protocol::CreateFileProposal {
                path: CanonicalPath("src/two_hunk.rs".to_string()),
                initial_content: Some(new_text.clone()),
            },
        )),
        target_ids: vec![target.target_id.clone()],
        required_capability: CapabilityId("editor.create_file".to_string()),
        rollback_step_ids: Vec::new(),
    };
    let proposal = WorkspaceProposal {
        proposal_id: ProposalId(200),
        principal: PrincipalId("p".to_string()),
        capability: CapabilityId("c".to_string()),
        correlation_id: CorrelationId(200),
        payload: ProposalPayload::Batch(BatchProposalPayload {
            batch_id: uuid::Uuid::from_u128(200),
            atomicity: ProposalBatchAtomicity::OrderedNonAtomic,
            rollback_policy: ProposalBatchRollbackPolicy::NotRequired,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: vec![target.clone()],
                omitted_target_count: 0,
                redaction_hints: Vec::new(),
            },
            items: vec![item],
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
            summary: "conservative hunk filter test".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    };

    let mut contents = HashMap::new();
    contents.insert("two-hunk-file".to_string(), (old_text, new_text));
    let surface = compute_proposal_diff_surface(&proposal, &contents);

    // The section for "two-hunk-file" must have at least 2 chunks (one per
    // separated change region).
    let section = surface
        .sections
        .iter()
        .find(|s| s.target_id.as_deref() == Some("two-hunk-file"))
        .expect("section for two-hunk-file must exist");
    assert!(
        section.chunks.len() >= 2,
        "expected ≥2 chunks from well-separated changes, got {}",
        section.chunks.len()
    );

    // Accept only the FIRST chunk — conservative policy must EXCLUDE the target.
    let partial_accept: HashSet<String> = section
        .chunks
        .iter()
        .take(1)
        .map(|c| c.chunk_id.clone())
        .collect();
    let result = filtered_batch_proposal_for_accepted_hunks(&proposal, &surface, &partial_accept);
    assert!(
        result.is_none(),
        "accepting fewer than all hunks in a target must exclude it (conservative policy)"
    );

    // Accepting ALL chunks must INCLUDE the target.
    let all_accept: HashSet<String> = section.chunks.iter().map(|c| c.chunk_id.clone()).collect();
    let result = filtered_batch_proposal_for_accepted_hunks(&proposal, &surface, &all_accept)
        .expect("accepting all hunks must include the target");
    let ProposalPayload::Batch(ref batch) = result.payload else {
        panic!("expected batch payload");
    };
    assert_eq!(batch.target_coverage.targets.len(), 1);
    assert_eq!(batch.target_coverage.targets[0].target_id, "two-hunk-file");
}

/// T5 (F5 apply-path): Full filtering chain — create proposal, compute diff
/// surface, record accepts via `ProposalHunkDispositionState`, and verify that
/// only the accepted targets survive in the filtered proposal.
///
/// This exercises the complete pipeline short of an actual filesystem apply:
/// proposal → diff surface → disposition state → filter → filtered proposal.
#[test]
fn full_filtering_chain_accepted_targets_only() {
    let proposal = five_file_batch_proposal();
    let contents = five_file_contents();
    let surface = compute_proposal_diff_surface(&proposal, &contents);

    // Use the disposition state to accept file-1 and file-3.
    let mut state = ProposalHunkDispositionState::new();
    for section in surface.sections.iter().filter(|s| {
        matches!(
            s.target_id.as_deref(),
            Some("target-file-1") | Some("target-file-3")
        )
    }) {
        for chunk in &section.chunks {
            state.set_hunk_disposition(
                proposal.proposal_id,
                chunk.chunk_id.clone(),
                DelegatedTaskProposalHunkDisposition::Accepted,
            );
        }
    }

    let accepted_ids = state.accepted_hunk_ids(proposal.proposal_id);
    let filtered = filtered_batch_proposal_for_accepted_hunks(&proposal, &surface, &accepted_ids)
        .expect("filtered proposal must be produced when accepted hunks are non-empty");

    let ProposalPayload::Batch(ref batch) = filtered.payload else {
        panic!("filtered proposal must be Batch");
    };

    // Only file-1 and file-3 have real changes in the diff surface; accepting
    // their chunks must produce exactly those two targets.
    let retained_ids: HashSet<&str> = batch
        .target_coverage
        .targets
        .iter()
        .map(|t| t.target_id.as_str())
        .collect();
    assert!(
        retained_ids.contains("target-file-1"),
        "target-file-1 must be retained after full-accept via disposition state"
    );
    assert!(
        retained_ids.contains("target-file-3"),
        "target-file-3 must be retained after full-accept via disposition state"
    );
    assert!(
        !retained_ids.contains("target-file-2"),
        "target-file-2 must be excluded (not accepted)"
    );
    assert!(
        !retained_ids.contains("target-file-4"),
        "target-file-4 must be excluded (no diff hunks — no chunks to accept)"
    );
    assert!(
        !retained_ids.contains("target-file-5"),
        "target-file-5 must be excluded (no diff hunks — no chunks to accept)"
    );
}

/// T3-E: `filtered_batch_proposal_for_accepted_hunks` returns only the targets
/// whose chunks were accepted via the diff surface.
#[test]
fn filter_by_accepted_hunks_retains_correct_targets() {
    let proposal = five_file_batch_proposal();
    let contents = five_file_contents();
    let surface = compute_proposal_diff_surface(&proposal, &contents);

    // Accept hunks only from target-file-1 and target-file-2.
    let accepted_hunk_ids: HashSet<String> = surface
        .sections
        .iter()
        .filter(|s| {
            matches!(
                s.target_id.as_deref(),
                Some("target-file-1") | Some("target-file-2")
            )
        })
        .flat_map(|s| s.chunks.iter().map(|c| c.chunk_id.clone()))
        .collect();

    let filtered =
        filtered_batch_proposal_for_accepted_hunks(&proposal, &surface, &accepted_hunk_ids)
            .expect("filtered proposal must be produced");

    let ProposalPayload::Batch(ref batch) = filtered.payload else {
        panic!("filtered proposal must be Batch");
    };
    let retained_ids: HashSet<&str> = batch
        .target_coverage
        .targets
        .iter()
        .map(|t| t.target_id.as_str())
        .collect();
    assert!(
        retained_ids.contains("target-file-1"),
        "target-file-1 must be retained"
    );
    assert!(
        retained_ids.contains("target-file-2"),
        "target-file-2 must be retained"
    );
    assert!(
        !retained_ids.contains("target-file-3"),
        "target-file-3 must be excluded"
    );
}

/// T3-F: `accepted_hunk_ids()` returns the correct set of accepted chunk IDs.
#[test]
fn accepted_hunk_ids_reflects_current_decisions() {
    let mut state = ProposalHunkDispositionState::new();
    let pid = ProposalId(5);

    state.set_hunk_disposition(
        pid,
        "chunk-A",
        DelegatedTaskProposalHunkDisposition::Accepted,
    );
    state.set_hunk_disposition(
        pid,
        "chunk-B",
        DelegatedTaskProposalHunkDisposition::Rejected,
    );
    state.set_hunk_disposition(
        pid,
        "chunk-C",
        DelegatedTaskProposalHunkDisposition::Accepted,
    );

    let ids = state.accepted_hunk_ids(pid);
    assert!(ids.contains("chunk-A"));
    assert!(
        !ids.contains("chunk-B"),
        "Rejected hunk must not appear in accepted set"
    );
    assert!(ids.contains("chunk-C"));
}

// ─── Task 4 tests ─────────────────────────────────────────────────────────────

/// T4-A: `ProposalEvidencePanel` can be constructed with structured fields (no
/// raw provider output).
#[test]
fn evidence_panel_carries_structured_fields_only() {
    use legion_protocol::{
        ProposalEvidencePanel, ProposalPrivacyLabel, ProposalProvenance, ProposalRiskLabel,
        TimestampMillis,
    };

    let panel = ProposalEvidencePanel {
        test_results: None,
        command_summaries: Vec::new(),
        context_manifest: None,
        risk_rules: Vec::new(),
        provenance: ProposalProvenance {
            created_at: TimestampMillis(1000),
            updated_at: TimestampMillis(2000),
            proposal_id: ProposalId(42),
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Low,
        },
    };

    // The panel has no raw provider output: no free-text pass-through field.
    assert!(panel.test_results.is_none());
    assert!(panel.command_summaries.is_empty());
    assert!(panel.context_manifest.is_none());
    assert!(panel.risk_rules.is_empty());
    assert_eq!(panel.provenance.proposal_id, ProposalId(42));
}

/// T4-B: `ProposalEvidencePanel` with populated fields retains all structured data.
#[test]
fn evidence_panel_with_test_results_and_commands() {
    use legion_protocol::{
        CommandSummary, ProposalEvidencePanel, ProposalPrivacyLabel, ProposalProvenance,
        ProposalRiskLabel, RiskRuleEvidence, TestResultsSummary, TimestampMillis,
    };

    let panel = ProposalEvidencePanel {
        test_results: Some(TestResultsSummary {
            total_count: 10,
            passed_count: 8,
            failed_count: 2,
            skipped_count: 0,
            run_id: "run-123".to_string(),
        }),
        command_summaries: vec![CommandSummary {
            command_class: "cargo-test".to_string(),
            exit_code: Some(0),
            redacted: false,
        }],
        context_manifest: None,
        risk_rules: vec![RiskRuleEvidence {
            rule_id: "rule.no_credentials".to_string(),
            triggered: false,
            rationale_label: "no credential patterns detected".to_string(),
        }],
        provenance: ProposalProvenance {
            created_at: TimestampMillis(100),
            updated_at: TimestampMillis(200),
            proposal_id: ProposalId(7),
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Medium,
        },
    };

    assert_eq!(panel.test_results.as_ref().unwrap().total_count, 10);
    assert_eq!(panel.test_results.as_ref().unwrap().passed_count, 8);
    assert_eq!(panel.command_summaries.len(), 1);
    assert!(!panel.command_summaries[0].redacted);
    assert_eq!(panel.risk_rules.len(), 1);
    assert!(!panel.risk_rules[0].triggered);
}
