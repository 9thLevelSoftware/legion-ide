# Phase 2 Proposal Mutation Substrate Evidence

Date: 2026-05-24

## Scope

This evidence records the accepted Phase 2 protocol-contract, architecture-documentation, and app-orchestration work for generalized proposal-mediated mutation. Stages 1A–1I cover lifecycle state management, deny-by-default validation, registered open-buffer text edits, closed-file create/delete/rename, multi-file workspace-edit execution, edit-only code-action execution, generic save-file apply with audit-before-success, batch preflight/contract planning, accepted reversible batch mutation with rollback checkpoints, rollback-after-audit-failure, app-owned lifecycle recovery snapshots, live proposal ledger projection, and workspace-authorized rollback checkpoints for accepted file mutations. Unsupported mutation classes remain deny-by-default, raw format requests must be lowered into `TextEdit` or `WorkspaceEdit` proposals, and placeholder runtime crates remain inactive.

## Acceptance status

- Phase 2 acceptance: Accepted.
- Generalized proposal execution acceptance: Accepted for save-file, open-buffer text edit, closed-file create/delete/rename, workspace-edit, edit-only code-action, and reversible batch routes with complete target coverage and rollback proof.
- Deferred beyond Phase 2: terminal command execution, plugin/runtime/remote/collaboration/AI mutation, command-bearing code actions, and raw `FormatFile` execution that has not been lowered into a proposal-safe edit payload.

## Architecture decision

- Added [`ADR-0016-generalized-proposal-service.md`](../../adrs/ADR-0016-generalized-proposal-service.md) to define the generalized proposal service boundary.
- The ADR records universal lifecycle states, explicit approve/reject/cancel/rollback lifecycle commands, batch and multi-file atomicity boundaries, rollback limits, deny-by-default validation, audit-before-success semantics, metadata-only persistence, and projection-only UI constraints.
- The current save baseline is preserved through [`SaveWorkflowService::save_active_buffer()`](../../../crates/legion-app/src/lib.rs:1314) and [`WorkspaceActor::save_file_with_proposal()`](../../../crates/legion-project/src/lib.rs:1622).
- Generic save-file proposal apply is now implemented through [`AppComposition::apply_save_file_proposal()`](../../../crates/legion-app/src/lib.rs) and routes through the same editor/workspace preconditions, with audit-before-success and rollback on audit failure.

## Protocol contract changes

Updated [`lib.rs`](../../../crates/legion-protocol/src/lib.rs) with metadata-first DTOs for generalized proposal mutation:

- Added [`ProposalPayload::Batch`](../../../crates/legion-protocol/src/lib.rs:1396) and [`ProposalPayloadKind::Batch`](../../../crates/legion-protocol/src/lib.rs:1830).
- Added [`BatchProposalPayload`](../../../crates/legion-protocol/src/lib.rs:1499) with deterministic item ordering, batch identifier, explicit atomicity, rollback policy, target coverage, dependency edges, rollback steps, partial-failure records, preview warnings, and schema version.
- Added [`ProposalBatchItem`](../../../crates/legion-protocol/src/lib.rs:1524), [`ProposalBatchDependency`](../../../crates/legion-protocol/src/lib.rs:1552), [`ProposalRollbackStep`](../../../crates/legion-protocol/src/lib.rs:1584), [`ProposalPartialFailureRecord`](../../../crates/legion-protocol/src/lib.rs:1620), and [`ProposalPreviewWarning`](../../../crates/legion-protocol/src/lib.rs:1652).
- Added affected-target coverage through [`ProposalAffectedTarget`](../../../crates/legion-protocol/src/lib.rs:1446) and [`ProposalTargetCoverage`](../../../crates/legion-protocol/src/lib.rs:1486), including open-buffer, closed-file, path-only, terminal, remote, collaboration, plugin, and metadata-only classifications.
- Added [`ProposalLifecycleCommand`](../../../crates/legion-protocol/src/lib.rs:2007), [`ProposalLifecycleAction`](../../../crates/legion-protocol/src/lib.rs:1973), and [`ProposalLifecycleCommandReason`](../../../crates/legion-protocol/src/lib.rs:1992).
- Extended [`ProposalRequest`](../../../crates/legion-protocol/src/lib.rs:2804) with approve, reject, cancel, and rollback commands while preserving validate, preview, and apply variants.
- Added [`ProposalLifecycleState::Cancelled`](../../../crates/legion-protocol/src/lib.rs:1859), [`ProposalCancellationReason`](../../../crates/legion-protocol/src/lib.rs:1918), and [`ProposalResponse::Cancelled`](../../../crates/legion-protocol/src/lib.rs:2882).
- Added rollback step DTOs and workspace-authorized rollback checkpoint records used for audit-failure rollback.

## App orchestration substrate

Updated [`lib.rs`](../../../crates/legion-app/src/lib.rs) with an app-level proposal coordinator and total affected-target routing:

- Replaced the save-only coordinator with [`AppProposalCoordinator`](../../../crates/legion-app/src/lib.rs:194) and proposal routing classifications in [`ProposalExecutionRoute`](../../../crates/legion-app/src/lib.rs:124).
- Added deterministic affected-target derivation through [`AppProposalCoordinator::affected_target_coverage()`](../../../crates/legion-app/src/lib.rs:350) and a total payload visitor in [`AppProposalCoordinator::visit_payload_targets()`](../../../crates/legion-app/src/lib.rs:373).
- The visitor covers text edits, create/delete/rename file payloads, save payloads, format/code-action payloads, terminal commands, and batches. Batch coverage preserves explicit target order when supplied and otherwise sorts items by item order and identifier before recursively visiting payloads.
- Replaced panic-prone save event extraction with optional save target derivation in [`save_event_target()`](../../../crates/legion-app/src/lib.rs:1734). Non-save stale or denied responses now fall back to generic proposal rejection events instead of assuming a file-backed save target.
- Unsupported non-save validation or apply attempts return structured unsupported rejections through [`AppProposalCoordinator::unsupported_response()`](../../../crates/legion-app/src/lib.rs:545) rather than panicking or surfacing internal errors.
- Explicit lifecycle request variants now produce typed lifecycle responses through [`ProposalPort`](../../../crates/legion-app/src/lib.rs:671) while non-save execution remains fail-closed.
- Exposed app orchestration entry points through [`AppComposition::handle_proposal_request()`](../../../crates/legion-app/src/lib.rs:2064) and [`AppComposition::proposal_target_coverage()`](../../../crates/legion-app/src/lib.rs:2074) without adding editor text/session ownership to the UI.
- Added audit-before-success wiring: `SaveWorkflowService::observe_proposal_response()` emits proposal audit and event metadata records; audit write failure causes `rollback_audit_failed_mutation()` to compensate side effects through editor/workspace authority.
- Added app-owned proposal ledger projection and lifecycle recovery snapshot support in `AppProposalCoordinator`; `AppComposition::shell_projection_snapshot()` now supplies live metadata-only proposal rows to the projection-only UI shell.

## Save invariants preserved and extended

- Existing durable saves still flow through [`AppComposition::save_active_buffer()`](../../../crates/legion-app/src/lib.rs:1959), [`SaveWorkflowService::save_active_buffer()`](../../../crates/legion-app/src/lib.rs:1314), and [`WorkspaceActor::save_file_with_proposal()`](../../../crates/legion-project/src/lib.rs:1622).
- Save proposals continue carrying expected disk fingerprint, file content version, workspace generation, buffer version, snapshot identifier, required write capability, principal, correlation identifier, and causality metadata.
- Stale/conflict/denied saves continue to return rejected app save outcomes rather than thrown errors, and dirty editor text remains preserved.
- External overwrite protection remains covered by [`workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict()`](../../../crates/legion-app/tests/workspace_vfs_integration.rs:402), including an assertion that the rejected response is stale.
- Generic save-file proposal apply is now covered by:
  - `workspace_vfs_integration_registered_save_apply_routes_through_workspace_actor`
  - `workspace_vfs_integration_stale_registered_save_preserves_dirty_buffer_and_disk`
  - `workspace_vfs_integration_conflicted_registered_save_preserves_dirty_buffer_and_disk`
  - `workspace_vfs_integration_registered_save_audit_failure_fails_closed_and_rolls_back`
  - `workspace_vfs_integration_oversized_save_is_rejected_and_preserves_dirty_text`

## Stage 1A lifecycle service update

Date: 2026-05-15

- `AppProposalCoordinator` now stores app-created lifecycle context and proposal lifecycle state keyed by `ProposalId` instead of accepting successful stateless lifecycle helper calls.
- Registered proposals enforce ordered lifecycle transitions for created, validated, previewed, approved, applied, rejected, denied, failed, rolled back, stale, conflict, and cancelled outcomes.
- Stateless preview and lifecycle command requests reject with `proposal.missing_lifecycle_context`; invalid ordering rejects with `proposal.invalid_lifecycle_transition`.
- Generic `ProposalPayload::SaveFile` apply is now implemented; it is no longer denied. The save proposal DTO still does not carry source text, so safe execution routes through `apply_save_file_proposal()` -> editor save request -> `WorkspaceActor::save_file_with_proposal()`, reusing the same editor/workspace/storage context without weakening preconditions.
- Focused tests added:
  - `proposal_coordinator_enforces_preview_after_validation`
  - `proposal_coordinator_rejects_command_without_lifecycle_context`
  - `proposal_coordinator_rejects_stateless_generic_save_apply`
  - `workspace_vfs_integration_unknown_lifecycle_context_preview_is_rejected`

## Stage 1B deny-by-default validation update

Date: 2026-05-15

- `AppProposalCoordinator::validate_proposal()` now evaluates every current `ProposalPayload` family rather than treating non-save validation as a panic-safe placeholder.
- Common validation now rejects missing principal, missing capability, zero correlation id, missing preview summary, incomplete target coverage, and missing affected targets before any apply path can be considered.
- Payload-specific validation now covers text edits, create/delete/rename, save, format, code action, workspace edit, terminal command, and batch payloads.
- Terminal, unsupported, and mixed routes still return structured `ProposalRejectionReason::Unsupported` before side effects.
- Registered non-save proposals with missing preconditions deny with `ProposalDenialReason::PolicyDenied`; stateless non-save proposals still reject with `proposal.missing_lifecycle_context`.
- Batch validation now checks schema version, items, complete target coverage, rollback policy/atomicity compatibility, required rollback steps, item ids, and item capabilities. Stage 1I supersedes the earlier runtime-denial-only posture for reversible batch routes that pass complete preflight and rollback proof.
- Focused tests added or updated:
  - `proposal_coordinator_denies_registered_text_edit_missing_preconditions`
  - `workspace_vfs_integration_non_save_proposals_are_structurally_rejected_without_panic`

## Stage 1C proposal apply execution update

Date: 2026-05-16

- `AppComposition::handle_proposal_request()` now routes registered `ProposalRequest::Apply` calls through app-owned executors after the proposal reaches `Previewed` or `Approved` lifecycle state.
- Open-buffer `ProposalPayload::TextEdit` applies through `EditorEngine::apply_protocol_edits()` using byte-coordinate protocol ranges, required buffer version, required snapshot id, editor-owned transactions, and `editor.transaction_applied` emission.
- Closed-file `CreateFile`, `DeleteFile`, and `RenameFile` apply through `WorkspaceActor::{create,delete,rename}_file_with_proposal()` and the platform filesystem port rather than direct app-level `std::fs` mutation.
- Workspace mutations require proposal lifecycle context, trusted `fs.write` capability, expected workspace generation, and file content/fingerprint preconditions where applicable. Open-file delete/rename is denied with `proposal.open_file_workspace_mutation_denied`.
- Batch apply remains fail-closed with structured `ProposalRejectionReason::Unsupported`. Generic save apply is now implemented and tested.
- Focused tests added:
  - `workspace_vfs_integration_registered_text_edit_apply_mutates_open_buffer`
  - `workspace_vfs_integration_stale_text_edit_precondition_does_not_apply`
  - `workspace_vfs_integration_closed_file_create_delete_rename_apply_through_workspace`
  - `workspace_vfs_integration_open_file_delete_and_rename_are_denied`
  - `workspace_vfs_integration_batch_apply_remains_fail_closed_after_preview`

## Stage 1D batch preflight/planning update

Date: 2026-05-16

- Added `AppComposition::preflight_batch_proposal()` with documented app-level DTOs for side-effect-free batch planning: `BatchPreflightPlan`, `BatchPreflightItemPlan`, and `BatchPreflightRoute`.
- The planner validates batch payload shape, deterministic item order, unique item ids, complete non-empty unique target coverage, item target references, dependency edges, dependency cycles, route support, and rollback/atomicity boundaries without calling apply helpers or workspace mutation methods.
- Supported Stage 1D preflight routes are open-buffer `TextEdit` and closed-file/path `CreateFile`, `DeleteFile`, and `RenameFile`. Nested batch, terminal, save, format, code action, workspace edit, plugin, remote, collaboration, mixed, and unsupported routes remain non-executable and produce diagnostics before side effects.
- Text edit preflight checks an open editor buffer, buffer version, snapshot id, optional file/workspace/fingerprint preconditions, and byte-coordinate range bounds against the current snapshot descriptor without mutating text.
- Create/delete/rename preflight reads current workspace tree metadata, checks workspace generation and file/fingerprint preconditions, verifies create/rename paths stay inside the active workspace root, denies open-file closed-file mutation, and rejects existing create/rename destinations from the tree or disk without touching disk.
- `AllOrNothing` and rollback `Required` require rollback step ids for every item, and those steps must resolve to the same item/target without unsupported actions or step diagnostics. `NotSupported` with stronger-than `OrderedNonAtomic` is rejected. Stage 1I lifts the earlier `runtime_apply_disabled` posture only for accepted reversible batch plans; unsupported or partial-failure routes remain fail-closed.
- Focused tests added:
  - `workspace_vfs_integration_batch_preflight_plans_supported_routes_without_side_effects`
  - `workspace_vfs_integration_batch_preflight_rejects_dependency_errors_without_mutation`
  - `workspace_vfs_integration_batch_preflight_rejects_missing_and_unknown_targets`
  - `workspace_vfs_integration_batch_preflight_rejects_unproven_rollback_boundaries`
  - `workspace_vfs_integration_batch_apply_remains_fail_closed_after_preview` now preflights before apply and still verifies fail-closed runtime behavior.

## Stage 1E batch execution safety contracts

Date: 2026-05-16

- Added `AppComposition::plan_batch_execution_contract()` with documented app-level DTOs for side-effect-free batch execution contracts: `BatchExecutionContract`, `BatchExecutionStageContract`, `BatchExecutionStage`, and `BatchExecutionItemContract`.
- The contract reuses `preflight_batch_proposal()` and records ordered Prepare, Preflight, Mutate, Commit, Audit, Finalize, and Rollback stages. Stage 1I provides accepted mutation and rollback evidence for supported reversible routes while the journal continues to expose audit-before-success, commit, finalize, and rollback readiness metadata.
- Contract diagnostics and preview warnings explicitly state that Stage 1E planning is not runtime execution, so consumers cannot confuse preflight/contract planning with mutation.
- Item contracts expose route, item id, target ids, preflight result, exact rollback-proof status, diagnostics, and partial-failure disposition in deterministic item order.
- Strengthened rollback proof validation for `AllOrNothing` and rollback `Required` batches. Rollback steps must still resolve exactly to the owning item and target, and now the rollback action must match the item route: text edits require `EditorUndoGroup`, create-file requires `DeleteCreatedFile`, delete-file requires `RecreateDeletedFile`, and rename-file requires `RenamePathBack`.
- Added deterministic dependency-blocked partial-failure records for downstream items that are not started because prerequisite items failed preflight. These records use `NotStarted` and are distinct from direct `FailedBeforeMutation` planning failures.
- Stage 1E kept `ProposalPayload::Batch` runtime apply fail-closed while planning and contract proof were added. Stage 1I supersedes that restriction for accepted reversible routes; contract planning itself remains side-effect-free.
- Focused tests added or updated:
  - `workspace_vfs_integration_batch_execution_contract_reports_audit_and_commit_barriers`
  - `workspace_vfs_integration_batch_contract_rejects_mismatched_rollback_action`
  - `workspace_vfs_integration_batch_contract_records_dependency_blocked_partial_failures`
  - `workspace_vfs_integration_batch_apply_remains_fail_closed_after_preview` now plans a successful contract first and still verifies unsupported apply leaves disk unchanged.

## Stage 1F additional execution and audit coverage

Date: 2026-05-16–2026-05-22

- Added `apply_save_file_proposal()` with full precondition checks (buffer version, snapshot id, file content version, workspace generation, expected fingerprint), editor save payload assembly, workspace proposal save routing, and deferred success acknowledgment.
- Added `apply_workspace_edit_proposal()` that delegates single-file create/delete/rename operations to the existing closed-file workspace apply helpers; multi-edit and non-trivial workspace edits remain unsupported.
- Added rollback snapshot capture before mutation: text edit, create file, delete file, rename file, save file, and workspace edit paths capture rollback state.
- Added `rollback_audit_failed_mutation()` with concrete rollback actions: undo transaction for open-buffer text edits, delete created files, restore deleted/saved files, and reverse renames.
- Added `refresh_workspace_after_audit_rollback()` to rebuild workspace tree state after rollback disk changes.
- Added `SaveWorkflowService::observe_proposal_response()` to emit metadata-only audit and event records for every proposal response; audit write failures trigger rollback.
- Added integration tests for audit failure rollback on open-buffer text edits, closed-file create/delete/rename, and registered save apply.
- Added integration tests for oversized save rejection and dirty text preservation.

## Stage 1I generalized execution acceptance

Date: 2026-05-24

- `AppComposition::apply_workspace_edit_proposal()` now executes multi-file workspace edits through editor/workspace authorities after preflighting every text edit and file operation. If a later item fails, already committed mutations are rolled back in reverse order through editor undo or workspace-authorized rollback checkpoints.
- `AppComposition::apply_code_action_proposal()` accepts edit-only code actions as open-buffer text mutations with the same version and snapshot preconditions as text-edit proposals. Command-only or mixed command execution remains outside Phase 2 and is not represented as direct mutation.
- `AppComposition::apply_batch_proposal()` now executes accepted reversible batch routes after complete preflight. It rejects `OrderedNonAtomic` runtime apply until exact runtime partial-failure records exist, captures rollback before each item, applies supported item routes through the existing proposal executors, and rolls back committed items on failure or audit-write failure.
- Batch self-approval remains impossible: registered proposals must reach `Previewed` or `Approved` before apply, and apply from `Created` is rejected without mutation.
- Raw `FormatFile` remains fail-closed unless a formatter or LSP layer lowers the result into a proposal-safe `TextEdit` or `WorkspaceEdit` payload with target coverage and version preconditions.

## Stage 1G lifecycle recovery and live projection

Date: 2026-05-22

- Added an app-owned proposal lifecycle recovery snapshot that captures remembered proposal envelopes, lifecycle states, and lifecycle event contexts so coordinator state can be reconstructed without giving UI editor/workspace authority.
- Added metadata-only proposal ledger row construction from `AppProposalCoordinator` state, including payload kind, lifecycle display, target coverage, risk/privacy labels, rollback availability, context summary, redacted diff summary, warnings, diagnostics, and redaction hints.
- Wired `AppComposition::shell_projection_snapshot()` to emit the live proposal ledger projection instead of an empty placeholder while keeping `legion-ui` projection-only.
- Added focused tests:
  - `proposal_coordinator_exports_and_recovers_lifecycle_snapshot`
  - `proposal_coordinator_builds_metadata_only_ledger_projection`
  - `workspace_vfs_integration_shell_projection_lists_live_registered_proposals`

## Stage 1H workspace-authorized rollback checkpoints

Date: 2026-05-22

- Added workspace-owned rollback checkpoint DTOs and APIs in `legion-project`: `WorkspaceMutationRollbackTarget`, `WorkspaceMutationRollbackCheckpoint`, `WorkspaceActor::rollback_checkpoint_for_file_mutation()`, and `WorkspaceActor::rollback_file_mutation_with_checkpoint()`.
- `AppComposition` now captures rollback material for accepted closed-file/save mutations through `WorkspaceActor` and compensates audit-failed create/delete/rename/save mutations through workspace authority instead of direct app-level `std::fs` read/write/remove/rename calls.
- Rollback compensation verifies the current rollback target still matches the workspace-tracked post-mutation fingerprint before deleting, overwriting, or renaming it, so external changes between mutation and audit-failure rollback are refused fail-closed.
- Open-buffer rollback remains editor-owned undo. Single-file workspace-edit delegation reuses the same workspace rollback checkpoints for create/delete/rename operations.
- Runtime batch mutation, batch rollback, multi-file workspace edits, and edit-only code-action execution are accepted for supported reversible routes with complete preflight and rollback proof. Raw format execution and future AI/plugin/remote/collaboration/LSP/terminal runtime routes remain denied unless they lower into accepted proposal payloads or pass a later ADR/policy/test gate.
- Focused tests added or rerun:
  - `rename_file_with_proposal_requires_destination_write_authorization`
  - `rollback_checkpoints_compensate_file_mutations_through_workspace_authority`
  - `rollback_checkpoint_refuses_to_clobber_external_changes`
  - `workspace_vfs_integration_rollback_audit_failure_records_failed_lifecycle`
  - `workspace_vfs_integration_closed_file_audit_failure_fails_closed_and_rolls_back`
  - `workspace_vfs_integration_registered_save_audit_failure_fails_closed_and_rolls_back`
  - `audit_rollback_failure_diagnostics_are_preserved_on_failed_response`

## Integration tests

Updated [`workspace_vfs_integration.rs`](../../../crates/legion-app/tests/workspace_vfs_integration.rs) with app-level Phase 2 regression coverage:

- [`workspace_vfs_integration_non_save_proposals_are_structurally_rejected_without_panic()`](../../../crates/legion-app/tests/workspace_vfs_integration.rs:472) covers text edit, create-file, and terminal-command proposals through validate/apply requests and asserts structured unsupported rejections.
- [`workspace_vfs_integration_batch_affected_targets_are_visited_in_item_order()`](../../../crates/legion-app/tests/workspace_vfs_integration.rs:515) verifies deterministic batch target derivation when explicit coverage is absent.
- [`workspace_vfs_integration_batch_uses_explicit_target_coverage_order()`](../../../crates/legion-app/tests/workspace_vfs_integration.rs:599) verifies explicit batch target coverage order is preserved.
- Stage 1C apply tests verify registered text-edit apply/stale behavior, closed-file workspace mutations, open-file mutation denial, and batch apply fail-closed behavior.
- Stage 1D/1E batch tests verify supported-route planning metadata, dependency and cycle diagnostics, missing/unknown target diagnostics, route-compatible rollback proof, audit-before-success/commit/finalize barriers, deterministic direct and dependency-blocked partial-failure records, and no disk/editor mutation during planning.
- Stage 1F/1H audit-rollback tests verify open-buffer text-edit undo, workspace-authorized closed-file create/delete/rename disk restoration, registered save rollback, audit-failure dirty-buffer preservation, failed ledger state after audit rollback, and fail-closed refusal to clobber external rollback-target changes.
- Stage 1G projection tests verify recoverable lifecycle state and live metadata-only proposal ledger rows in the shell snapshot.
- Existing stale/conflict, denial, failed-save, and dirty-buffer preservation tests remain in the same integration suite.

## Placeholder and dependency boundaries

- Did not activate [`legion-agent`](../../../crates/legion-agent/src/lib.rs), [`legion-index`](../../../crates/legion-index/src/lib.rs), [`legion-memory`](../../../crates/legion-memory/src/lib.rs), or [`legion-tracker`](../../../crates/legion-tracker/src/lib.rs).
- Did not add dependencies or change dependency policy.
- Did not add a direct [`legion-project`](../../../crates/legion-project/src/lib.rs) dependency to [`legion-editor`](../../../crates/legion-editor/src/lib.rs).
- Did not change [`legion-ui`](../../../crates/legion-ui/src/ui.rs) projection-only ownership rules.

## Deferred beyond Phase 2

- No remaining acceptance gap blocks Phase 2.
- Raw `FormatFile` requests remain denied unless lowered into a `TextEdit` or `WorkspaceEdit` proposal by a formatter/LSP layer with full target coverage and preconditions.
- Command-bearing code actions, terminal commands, AI, plugin, LSP process, collaboration, and remote runtime apply paths remain denied until their ADR, dependency-policy, and contract-test gates exist.

## Validation

Completed targeted validation retained from earlier Phase 2 subtasks plus Stage 1G/1H:

- `cargo fmt --all` — passed on 2026-05-22.
- `cargo check -p legion-app --all-targets` — passed.
- `cargo test -p legion-app --test workspace_vfs_integration` — passed with 51 integration tests; 0 failed; 0 ignored.
- `cargo clippy -p legion-app --all-targets -- -D warnings` — passed.
- `cargo test -p legion-protocol --test dto_contracts` — passed with 56 tests; 0 failed; 0 ignored.
- `cargo check -p legion-app -p legion-observability --all-targets` — passed.
- `cargo test -p legion-app --lib` — passed with 18 unit tests; 0 failed; 0 ignored.
- `cargo test -p legion-project rollback_checkpoints_compensate_file_mutations_through_workspace_authority` — passed.
- `cargo test -p legion-project rename_file_with_proposal_requires_destination_write_authorization` — passed.
- `cargo test -p legion-project rollback_checkpoint_refuses_to_clobber_external_changes` — passed.
- `cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_closed_file_audit_failure_fails_closed_and_rolls_back` — passed.
- `cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_registered_save_audit_failure_fails_closed_and_rolls_back` — passed.
- `cargo test -p legion-app --lib audit_rollback_failure_diagnostics_are_preserved_on_failed_response` — passed.
- `cargo clippy -p legion-project --all-targets -- -D warnings` — passed.
- `cargo clippy -p legion-app --all-targets -- -D warnings` — passed.

Completed Stage 1I acceptance validation on 2026-05-24:

- `cargo test -p legion-app --test workspace_vfs_integration` — passed with 54 integration tests; 0 failed; 0 ignored.
- The suite includes `workspace_vfs_integration_workspace_edit_multi_file_text_edits_apply_atomically`, `workspace_vfs_integration_code_action_edits_apply_through_proposal_executor`, `workspace_vfs_integration_batch_ordered_non_atomic_requires_partial_failures_before_apply`, `workspace_vfs_integration_batch_apply_cannot_self_approve_from_created_state`, and the existing save conflict/dirty-buffer preservation regressions.

Completed full repository phase gates on 2026-05-22 from the workspace root with `C:/Users/dasbl/.cargo/bin` first in `PATH`:

| Gate | Command | Result |
| --- | --- | --- |
| Dependency policy | `cargo run -p xtask -- check-deps` | Passed; `dependency policy checks passed`. |
| Formatting | `cargo fmt --all --check` | Passed. |
| Workspace check | `cargo check --workspace --all-targets` | Passed. |
| Workspace tests | `cargo test --workspace --all-targets` | Passed; observed test binaries reported 313 passed, 0 failed, and 3 ignored performance-suite measurements. |
| Workspace clippy | `cargo clippy --workspace --all-targets -- -D warnings` | Passed. |

No placeholder crates were activated, and no runtime behavior was added for AI, plugins, collaboration, remote workspaces, terminal runtime, LSP execution, or batch rollback.
