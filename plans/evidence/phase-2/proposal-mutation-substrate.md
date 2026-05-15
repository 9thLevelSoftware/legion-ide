# Phase 2 Proposal Mutation Substrate Evidence

Date: 2026-05-15

## Scope

This evidence records the Phase 2 protocol-contract, architecture-documentation, and app-orchestration substrate work for generalized proposal-mediated mutation. Runtime execution remains deny-by-default for non-save mutation classes, and placeholder runtime crates remain inactive.

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

## Integration tests

Updated [`workspace_vfs_integration.rs`](../../../crates/devil-app/tests/workspace_vfs_integration.rs) with app-level Phase 2 regression coverage:

- [`workspace_vfs_integration_non_save_proposals_are_structurally_rejected_without_panic()`](../../../crates/devil-app/tests/workspace_vfs_integration.rs:472) covers text edit, create-file, and terminal-command proposals through validate/apply requests and asserts structured unsupported rejections.
- [`workspace_vfs_integration_batch_affected_targets_are_visited_in_item_order()`](../../../crates/devil-app/tests/workspace_vfs_integration.rs:515) verifies deterministic batch target derivation when explicit coverage is absent.
- [`workspace_vfs_integration_batch_uses_explicit_target_coverage_order()`](../../../crates/devil-app/tests/workspace_vfs_integration.rs:599) verifies explicit batch target coverage order is preserved.
- Existing stale/conflict, denial, failed-save, and dirty-buffer preservation tests remain in the same integration suite.

## Placeholder and dependency boundaries

- Did not activate [`devil-agent`](../../../crates/devil-agent/src/lib.rs), [`devil-index`](../../../crates/devil-index/src/lib.rs), [`devil-memory`](../../../crates/devil-memory/src/lib.rs), or [`devil-tracker`](../../../crates/devil-tracker/src/lib.rs).
- Did not add dependencies or change dependency policy.
- Did not add a direct [`devil-project`](../../../crates/devil-project/src/lib.rs) dependency to [`devil-editor`](../../../crates/devil-editor/src/lib.rs).
- Did not change [`devil-ui`](../../../crates/devil-ui/src/ui.rs) projection-only ownership rules.

## Remaining Phase 2 gaps

- Runtime apply planning beyond saves remains denied until follow-on work separates validation, preview, approval, preflight, apply, commit, rollback, and audit emission for each mutation class.
- Open-buffer edit execution through editor-owned transactions and closed-file mutation execution through workspace VFS authority remain future implementation work.
- Future AI, plugin, LSP, collaboration, terminal, and remote runtime apply paths remain denied until their ADR, dependency-policy, and contract-test gates exist.

## Validation

Completed targeted validation retained from earlier Phase 2 subtasks:

- `cargo check -p devil-app --all-targets` — passed.
- `cargo test -p devil-app --test workspace_vfs_integration` — passed with 15 tests; 0 failed; 0 ignored.
- `cargo fmt --all` — passed.
- `cargo test -p devil-protocol --test dto_contracts` — passed with 14 tests; 0 failed; 0 ignored.
- `cargo check -p devil-app -p devil-observability --all-targets` — passed.

Completed full repository phase gates on 2026-05-15 from the workspace root:

| Gate | Command | Result |
| --- | --- | --- |
| Dependency policy | `cargo run -p xtask -- check-deps` | Passed; `dependency policy checks passed`. |
| Formatting | `cargo fmt --all --check` | Passed. |
| Workspace check | `cargo check --workspace --all-targets` | Passed. |
| Workspace tests | `cargo test --workspace --all-targets` | Passed; observed test binaries reported 152 passed, 0 failed, and 3 ignored performance-suite measurements. |
| Workspace clippy | `cargo clippy --workspace --all-targets -- -D warnings` | Initial required-command attempt failed for a non-Phase-2 environment reason: `cargo` and `rustc` resolved to Chocolatey toolchain 1.93.1 while `cargo-clippy` resolved to toolchain 1.95.0, producing E0514 incompatible-compiler metadata errors before Phase 2 code was linted. After forcing a consistent 1.95.0 toolchain with `set "PATH=C:\Users\dasbl\.cargo\bin;%PATH%"`, using isolated target directory `target-clippy-rustc195`, and cleaning that target, the same clippy arguments passed. |

No source remediation was required. The only remediation performed was build-environment cleanup/toolchain-path normalization for clippy; no runtime apply behavior was broadened, and no placeholder crates were activated.
