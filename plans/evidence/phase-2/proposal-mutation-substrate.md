# Phase 2 Proposal Mutation Substrate Evidence

Date: 2026-05-15

## Scope

This evidence records the Phase 2 protocol-contract, architecture-documentation, and app-orchestration substrate work for generalized proposal-mediated mutation. Stage 1C enables registered open-buffer text edits and closed-file create/delete/rename execution through existing editor/workspace authorities; unsupported mutation classes remain deny-by-default, and placeholder runtime crates remain inactive.

## Architecture decision

- Added [`ADR-0016-generalized-proposal-service.md`](../../adrs/ADR-0016-generalized-proposal-service.md) to define the generalized proposal service boundary.
- The ADR records universal lifecycle states, explicit approve/reject/cancel/rollback lifecycle commands, batch and multi-file atomicity boundaries, rollback limits, deny-by-default validation, audit-before-success semantics, metadata-only persistence, and projection-only UI constraints.
- The current save baseline is preserved through [`SaveWorkflowService::save_active_buffer()`](../../../crates/devil-app/src/lib.rs:1314) and [`WorkspaceActor::save_file_with_proposal()`](../../../crates/devil-project/src/lib.rs:1622).

## Protocol contract changes

Updated [`lib.rs`](../../../crates/devil-protocol/src/lib.rs) with metadata-first DTOs for generalized proposal mutation:

- Added [`ProposalPayload::Batch`](../../../crates/devil-protocol/src/lib.rs:1396) and [`ProposalPayloadKind::Batch`](../../../crates/devil-protocol/src/lib.rs:1830).
- Added [`BatchProposalPayload`](../../../crates/devil-protocol/src/lib.rs:1499) with deterministic item ordering, batch identifier, explicit atomicity, rollback policy, target coverage, dependency edges, rollback steps, partial-failure records, preview warnings, and schema version.
- Added [`ProposalBatchItem`](../../../crates/devil-protocol/src/lib.rs:1524), [`ProposalBatchDependency`](../../../crates/devil-protocol/src/lib.rs:1552), [`ProposalRollbackStep`](../../../crates/devil-protocol/src/lib.rs:1584), [`ProposalPartialFailureRecord`](../../../crates/devil-protocol/src/lib.rs:1620), and [`ProposalPreviewWarning`](../../../crates/devil-protocol/src/lib.rs:1652).
- Added affected-target coverage through [`ProposalAffectedTarget`](../../../crates/devil-protocol/src/lib.rs:1446) and [`ProposalTargetCoverage`](../../../crates/devil-protocol/src/lib.rs:1486), including open-buffer, closed-file, path-only, terminal, remote, collaboration, plugin, and metadata-only classifications.
- Added [`ProposalLifecycleCommand`](../../../crates/devil-protocol/src/lib.rs:2007), [`ProposalLifecycleAction`](../../../crates/devil-protocol/src/lib.rs:1973), and [`ProposalLifecycleCommandReason`](../../../crates/devil-protocol/src/lib.rs:1992).
- Extended [`ProposalRequest`](../../../crates/devil-protocol/src/lib.rs:2804) with approve, reject, cancel, and rollback commands while preserving validate, preview, and apply variants.
- Added [`ProposalLifecycleState::Cancelled`](../../../crates/devil-protocol/src/lib.rs:1859), [`ProposalCancellationReason`](../../../crates/devil-protocol/src/lib.rs:1918), and [`ProposalResponse::Cancelled`](../../../crates/devil-protocol/src/lib.rs:2882).

## App orchestration substrate

Updated [`lib.rs`](../../../crates/devil-app/src/lib.rs) with an app-level proposal coordinator and total affected-target routing:

- Replaced the save-only coordinator with [`AppProposalCoordinator`](../../../crates/devil-app/src/lib.rs:194) and proposal routing classifications in [`ProposalExecutionRoute`](../../../crates/devil-app/src/lib.rs:124).
- Added deterministic affected-target derivation through [`AppProposalCoordinator::affected_target_coverage()`](../../../crates/devil-app/src/lib.rs:350) and a total payload visitor in [`AppProposalCoordinator::visit_payload_targets()`](../../../crates/devil-app/src/lib.rs:373).
- The visitor covers text edits, create/delete/rename file payloads, save payloads, format/code-action payloads, terminal commands, and batches. Batch coverage preserves explicit target order when supplied and otherwise sorts items by item order and identifier before recursively visiting payloads.
- Replaced panic-prone save event extraction with optional save target derivation in [`save_event_target()`](../../../crates/devil-app/src/lib.rs:1734). Non-save stale or denied responses now fall back to generic proposal rejection events instead of assuming a file-backed save target.
- Unsupported non-save validation or apply attempts return structured unsupported rejections through [`AppProposalCoordinator::unsupported_response()`](../../../crates/devil-app/src/lib.rs:545) rather than panicking or surfacing internal errors.
- Explicit lifecycle request variants now produce typed lifecycle responses through [`ProposalPort`](../../../crates/devil-app/src/lib.rs:671) while non-save execution remains fail-closed.
- Exposed app orchestration entry points through [`AppComposition::handle_proposal_request()`](../../../crates/devil-app/src/lib.rs:2064) and [`AppComposition::proposal_target_coverage()`](../../../crates/devil-app/src/lib.rs:2074) without adding editor text/session ownership to the UI.

## Save invariants preserved

- Existing durable saves still flow through [`AppComposition::save_active_buffer()`](../../../crates/devil-app/src/lib.rs:1959), [`SaveWorkflowService::save_active_buffer()`](../../../crates/devil-app/src/lib.rs:1314), and [`WorkspaceActor::save_file_with_proposal()`](../../../crates/devil-project/src/lib.rs:1622).
- Save proposals continue carrying expected disk fingerprint, file content version, workspace generation, buffer version, snapshot identifier, required write capability, principal, correlation identifier, and causality metadata.
- Stale/conflict/denied saves continue to return rejected app save outcomes rather than thrown errors, and dirty editor text remains preserved.
- External overwrite protection remains covered by [`workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict()`](../../../crates/devil-app/tests/workspace_vfs_integration.rs:402), including an assertion that the rejected response is stale.

## Stage 1A lifecycle service update

Date: 2026-05-15

- `AppProposalCoordinator` now stores app-created lifecycle context and proposal lifecycle state keyed by `ProposalId` instead of accepting successful stateless lifecycle helper calls.
- Registered proposals enforce ordered lifecycle transitions for created, validated, previewed, approved, applied, rejected, denied, failed, rolled back, stale, conflict, and cancelled outcomes.
- Stateless preview and lifecycle command requests reject with `proposal.missing_lifecycle_context`; invalid ordering rejects with `proposal.invalid_lifecycle_transition`.
- Generic `ProposalPayload::SaveFile` apply remains denied with an explicit migration rationale. The save proposal DTO does not carry source text, so safe execution must continue through `AppComposition::save_active_buffer()` -> `SaveWorkflowService` -> `WorkspaceActor::save_file_with_proposal()` until a generic app-level executor can reuse the same editor/workspace/storage context without weakening preconditions.
- Focused tests added:
  - `proposal_coordinator_enforces_preview_after_validation`
  - `proposal_coordinator_rejects_command_without_lifecycle_context`
  - `proposal_coordinator_documents_generic_save_apply_denial`
  - `workspace_vfs_integration_unknown_lifecycle_context_preview_is_rejected`

## Stage 1B deny-by-default validation update

Date: 2026-05-15

- `AppProposalCoordinator::validate_proposal()` now evaluates every current `ProposalPayload` family rather than treating non-save validation as a panic-safe placeholder.
- Common validation now rejects missing principal, missing capability, zero correlation id, missing preview summary, incomplete target coverage, and missing affected targets before any apply path can be considered.
- Payload-specific validation now covers text edits, create/delete/rename, save, format, code action, workspace edit, terminal command, and batch payloads.
- Terminal, unsupported, and mixed routes still return structured `ProposalRejectionReason::Unsupported` before side effects.
- Registered non-save proposals with missing preconditions deny with `ProposalDenialReason::PolicyDenied`; stateless non-save proposals still reject with `proposal.missing_lifecycle_context`.
- Batch validation now checks schema version, items, complete target coverage, rollback policy/atomicity compatibility, required rollback steps, item ids, and item capabilities. Runtime batch apply remains denied.
- Focused tests added or updated:
  - `proposal_coordinator_denies_registered_text_edit_missing_preconditions`
  - `workspace_vfs_integration_non_save_proposals_are_structurally_rejected_without_panic`

## Stage 1C proposal apply execution update

Date: 2026-05-16

- `AppComposition::handle_proposal_request()` now routes registered `ProposalRequest::Apply` calls through app-owned executors after the proposal reaches `Previewed` or `Approved` lifecycle state.
- Open-buffer `ProposalPayload::TextEdit` applies through `EditorEngine::apply_protocol_edits()` using byte-coordinate protocol ranges, required buffer version, required snapshot id, editor-owned transactions, and `editor.transaction_applied` emission.
- Closed-file `CreateFile`, `DeleteFile`, and `RenameFile` apply through `WorkspaceActor::{create,delete,rename}_file_with_proposal()` and the platform filesystem port rather than direct app-level `std::fs` mutation.
- Workspace mutations require proposal lifecycle context, trusted `fs.write` capability, expected workspace generation, and file content/fingerprint preconditions where applicable. Open-file delete/rename is denied with `proposal.open_file_workspace_mutation_denied`.
- Batch apply remains fail-closed with structured `ProposalRejectionReason::Unsupported`; generic save apply still remains denied in favor of `AppComposition::save_active_buffer()`.
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
- `AllOrNothing` and rollback `Required` require rollback step ids for every item, and those steps must resolve to the same item/target without unsupported actions or step diagnostics. `NotSupported` with stronger-than `OrderedNonAtomic` is rejected. `runtime_apply_disabled` remains true for every plan, so this closes planning/preflight gaps only; runtime batch mutation, commit, and rollback are still denied.
- Focused tests added:
  - `workspace_vfs_integration_batch_preflight_plans_supported_routes_without_side_effects`
  - `workspace_vfs_integration_batch_preflight_rejects_dependency_errors_without_mutation`
  - `workspace_vfs_integration_batch_preflight_rejects_missing_and_unknown_targets`
  - `workspace_vfs_integration_batch_preflight_rejects_unproven_rollback_boundaries`
  - `workspace_vfs_integration_batch_apply_remains_fail_closed_after_preview` now preflights before apply and still verifies fail-closed runtime behavior.

## Stage 1E batch execution safety contracts

Date: 2026-05-16

- Added `AppComposition::plan_batch_execution_contract()` with documented app-level DTOs for side-effect-free batch execution contracts: `BatchExecutionContract`, `BatchExecutionStageContract`, `BatchExecutionStage`, and `BatchExecutionItemContract`.
- The contract reuses `preflight_batch_proposal()` and records ordered Prepare, Preflight, Mutate, Commit, Audit, Finalize, and Rollback stages. Mutate, Commit, Audit, Finalize, and Rollback remain blocked because runtime batch mutation is disabled and audit-before-success proof is not yet available.
- Contract diagnostics and preview warnings explicitly state that Stage 1E planning is not runtime execution, so consumers cannot confuse preflight/contract planning with mutation.
- Item contracts expose route, item id, target ids, preflight result, exact rollback-proof status, diagnostics, and partial-failure disposition in deterministic item order.
- Strengthened rollback proof validation for `AllOrNothing` and rollback `Required` batches. Rollback steps must still resolve exactly to the owning item and target, and now the rollback action must match the item route: text edits require `EditorUndoGroup`, create-file requires `DeleteCreatedFile`, delete-file requires `RecreateDeletedFile`, and rename-file requires `RenamePathBack`.
- Added deterministic dependency-blocked partial-failure records for downstream items that are not started because prerequisite items failed preflight. These records use `NotStarted` and are distinct from direct `FailedBeforeMutation` planning failures.
- `ProposalPayload::Batch` runtime apply remains fail-closed/unsupported, `BatchPreflightPlan.runtime_apply_disabled` remains true, and no editor/workspace mutation helpers are called from batch contract planning.
- Focused tests added or updated:
  - `workspace_vfs_integration_batch_execution_contract_reports_audit_and_commit_barriers`
  - `workspace_vfs_integration_batch_contract_rejects_mismatched_rollback_action`
  - `workspace_vfs_integration_batch_contract_records_dependency_blocked_partial_failures`
  - `workspace_vfs_integration_batch_apply_remains_fail_closed_after_preview` now plans a successful contract first and still verifies unsupported apply leaves disk unchanged.

## Integration tests

Updated [`workspace_vfs_integration.rs`](../../../crates/devil-app/tests/workspace_vfs_integration.rs) with app-level Phase 2 regression coverage:

- [`workspace_vfs_integration_non_save_proposals_are_structurally_rejected_without_panic()`](../../../crates/devil-app/tests/workspace_vfs_integration.rs:472) covers text edit, create-file, and terminal-command proposals through validate/apply requests and asserts structured unsupported rejections.
- [`workspace_vfs_integration_batch_affected_targets_are_visited_in_item_order()`](../../../crates/devil-app/tests/workspace_vfs_integration.rs:515) verifies deterministic batch target derivation when explicit coverage is absent.
- [`workspace_vfs_integration_batch_uses_explicit_target_coverage_order()`](../../../crates/devil-app/tests/workspace_vfs_integration.rs:599) verifies explicit batch target coverage order is preserved.
- Stage 1C apply tests verify registered text-edit apply/stale behavior, closed-file workspace mutations, open-file mutation denial, and batch apply fail-closed behavior.
- Stage 1D/1E batch tests verify supported-route planning metadata, dependency and cycle diagnostics, missing/unknown target diagnostics, route-compatible rollback proof, audit-before-success/commit/finalize barriers, deterministic direct and dependency-blocked partial-failure records, and no disk/editor mutation during planning.
- Existing stale/conflict, denial, failed-save, and dirty-buffer preservation tests remain in the same integration suite.

## Placeholder and dependency boundaries

- Did not activate [`devil-agent`](../../../crates/devil-agent/src/lib.rs), [`devil-index`](../../../crates/devil-index/src/lib.rs), [`devil-memory`](../../../crates/devil-memory/src/lib.rs), or [`devil-tracker`](../../../crates/devil-tracker/src/lib.rs).
- Did not add dependencies or change dependency policy.
- Did not add a direct [`devil-project`](../../../crates/devil-project/src/lib.rs) dependency to [`devil-editor`](../../../crates/devil-editor/src/lib.rs).
- Did not change [`devil-ui`](../../../crates/devil-ui/src/ui.rs) projection-only ownership rules.

## Remaining Phase 2 gaps

- Runtime apply planning beyond manual saves, registered open-buffer text edits, closed-file create/delete/rename, and side-effect-free batch preflight/contract planning remains denied until follow-on work implements apply, commit, rollback, and audit emission for each mutation class.
- Generic save apply remains intentionally denied until it can reuse the manual save workflow's editor text extraction, workspace preconditions, audit-before-success storage, and dirty-buffer preservation semantics.
- Runtime batch mutation, batch rollback, multi-file atomicity, workspace-edit execution, format/code-action execution, and generic save execution remain future implementation work after the Stage 1E contract paths.
- Future AI, plugin, LSP, collaboration, terminal, and remote runtime apply paths remain denied until their ADR, dependency-policy, and contract-test gates exist.

## Validation

Completed targeted validation retained from earlier Phase 2 subtasks plus Stage 1D:

- `cargo fmt --all` — passed on 2026-05-16 after Stage 1E edits.
- `cargo check -p devil-app --all-targets` — passed after Stage 1E edits.
- `cargo test -p devil-app --test workspace_vfs_integration` — passed with 32 integration tests; 0 failed; 0 ignored.
- `cargo clippy -p devil-app --all-targets -- -D warnings` — passed after Stage 1E edits.
- `cargo test -p devil-protocol --test dto_contracts` — passed with 14 tests; 0 failed; 0 ignored.
- `cargo check -p devil-app -p devil-observability --all-targets` — passed.

Completed full repository phase gates on 2026-05-16 from the workspace root with `C:/Users/dasbl/.cargo/bin` first in `PATH`:

| Gate | Command | Result |
| --- | --- | --- |
| Dependency policy | `cargo run -p xtask -- check-deps` | Passed; `dependency policy checks passed`. |
| Formatting | `cargo fmt --all --check` | Passed. |
| Workspace check | `cargo check --workspace --all-targets` | Passed. |
| Workspace tests | `cargo test --workspace --all-targets` | Passed; observed test binaries reported 187 passed, 0 failed, and 3 ignored performance-suite measurements. |
| Workspace clippy | `cargo clippy --workspace --all-targets -- -D warnings` | Passed. |

No placeholder crates were activated, and no runtime behavior was added for AI, plugins, collaboration, remote workspaces, terminal runtime, LSP execution, or batch rollback.
