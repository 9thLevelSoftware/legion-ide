# WS07.T1 Apply-activation audit

Date: 2026-06-11

## Summary

This checklist maps every current mutation route to the gate(s) that keep it safe and the test(s) that verify that gate. The current implementation keeps runtime batch apply disabled by default for untrusted workspaces and surfaces that state in the batch preflight plan, execution contract, and execution journal.

## Activation ADR checklist

| Route | Current gate(s) | Test evidence |
| --- | --- | --- |
| Save | Proposal lifecycle validation, save preconditions, expected fingerprint, workspace generation, dirty-buffer preservation, audit-before-success | `workspace_vfs_integration_untrusted_save_is_denied_without_disk_mutation`, `workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict`, `workspace_vfs_integration_oversized_save_is_rejected_and_preserves_dirty_text` |
| Text edit | Open-buffer identity, buffer version, snapshot id, stale-precondition rejection, editor transaction apply, undo-group rollback | `workspace_vfs_integration_registered_text_edit_apply_mutates_open_buffer`, `workspace_vfs_integration_stale_text_edit_precondition_does_not_apply` |
| Closed-file create/delete/rename | Workspace trust, path policy, expected fingerprint / generation, capability, conflict detection, rollback checkpoint | `workspace_vfs_integration_closed_file_create_delete_rename_apply_through_workspace`, `workspace_vfs_integration_untrusted_closed_file_create_is_denied_without_dirty_buffer_loss` |
| Workspace edit | Target coverage completeness, per-item route support, per-item preconditions, atomic rollback on failure | `workspace_vfs_integration_single_file_workspace_edit_create_applies_closed_file_only`, `workspace_vfs_integration_workspace_edit_multi_file_text_edits_apply_atomically` |
| Code action | Edit-only payload lowering, open-buffer preconditions, no command execution | `workspace_vfs_integration_code_action_edits_apply_through_proposal_executor`, `workspace_vfs_integration_format_file_requires_lowered_workspace_edit` |
| Batch | Deterministic preflight, rollback proof validation, target coverage, dependency ordering, runtime apply gating | `workspace_vfs_integration_batch_preflight_plans_supported_routes_without_side_effects`, `workspace_vfs_integration_batch_preflight_disables_runtime_apply_for_untrusted_workspace`, `workspace_vfs_integration_batch_execution_contract_reports_audit_and_commit_barriers`, `workspace_vfs_integration_batch_apply_cannot_self_approve_from_created_state` |

## Runtime apply default

`BatchPreflightPlan.runtime_apply_disabled`, `BatchExecutionContract.runtime_apply_disabled`, and `BatchExecutionJournal.runtime_apply_disabled` now derive from the active workspace trust state. Trusted workspaces keep batch runtime apply enabled; untrusted workspaces default to disabled and emit a metadata-only preview warning (`proposal.batch_runtime_apply_requires_trusted_workspace`).

## Verification commands

- `cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_batch_preflight_disables_runtime_apply_for_untrusted_workspace -- --exact --nocapture`
- `cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_batch_preflight_plans_supported_routes_without_side_effects -- --exact --nocapture`
- `cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_batch_execution_contract_reports_audit_and_commit_barriers -- --exact --nocapture`
- `cargo fmt --all --check`

## Evidence notes

- Trusted batch preflight remains side-effect-free and continues to report `runtime_apply_disabled = false`.
- Untrusted batch preflight now reports `runtime_apply_disabled = true` without introducing structural preflight failures.
- Execution contract and journal mirror the same trust-gated default so the audit surface and the runtime planning surface stay aligned.
