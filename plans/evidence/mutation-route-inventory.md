# Mutation Route Inventory

Date: 2026-06-14

This inventory records every known mutation route currently surfaced in the codebase and the current activation state of each route family. The goal is to make the authority boundary clear without overstating product readiness.

## Inventory

| Route | Current activation state | Current gate(s) | Evidence |
| --- | --- | --- | --- |
| Manual save | Active and proposal-mediated | `SaveWorkflowService::save_active_buffer()` routes through `WorkspaceActor::save_file_with_proposal()`; trust, fingerprint, version, and conflict checks fail closed | `crates/legion-app/src/lib.rs`, `crates/legion-project/src/lib.rs`, `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md` |
| Text edit | Active | Open-buffer identity, buffer version, snapshot id, lifecycle validation, and undo/rollback semantics | `crates/legion-app/tests/workspace_vfs_integration.rs` (`workspace_vfs_integration_registered_text_edit_apply_mutates_open_buffer`, `workspace_vfs_integration_stale_text_edit_precondition_does_not_apply`, `workspace_vfs_integration_text_edit_apply_requires_valid_lifecycle`) |
| Closed-file mutation | Active | Workspace VFS authority plus proposal lifecycle context; create/delete/rename are checkpointed and fail closed on rejection | `crates/legion-project/src/lib.rs`, `crates/legion-app/tests/workspace_vfs_integration.rs` (`workspace_vfs_integration_closed_file_create_delete_rename_apply_through_workspace`, `workspace_vfs_integration_untrusted_closed_file_create_is_denied_without_dirty_buffer_loss`) |
| Workspace edit | Active | Proposal-ready workspace edits from LSP/semantic tooling; atomic apply and rollback semantics | `crates/legion-protocol/src/lib.rs` (`WorkspaceEditProposalPayload`, `WorkspaceEditSourceKind`), `crates/legion-app/tests/workspace_vfs_integration.rs` (`workspace_vfs_integration_single_file_workspace_edit_create_applies_closed_file_only`, `workspace_vfs_integration_workspace_edit_multi_file_text_edits_apply_atomically`) |
| LSP action | Active, but trust-gated and lowered into proposals | `LspLaunchPolicy` requires trusted workspaces and an allowlist; formatting/rename/code action routes are represented as proposal sources rather than direct writes | `crates/legion-security/src/lib.rs`, `crates/legion-protocol/src/lib.rs` (`WorkspaceEditSourceKind::{LspRename,LspFormatting,LspCodeAction,StructuralSearchReplace}`), `crates/legion-app/tests/workspace_vfs_integration.rs` (`workspace_vfs_integration_code_action_edits_apply_through_proposal_executor`, `workspace_vfs_integration_format_file_requires_lowered_workspace_edit`) |
| AI proposal | Active for assisted/local flows | Trusted-loopback provider invocation is allowed; remote/cloud provider invocation remains denied by default; AI outputs become proposal payloads | `crates/legion-ai/src/lib.rs`, `crates/legion-security/src/lib.rs` (`AiProviderPolicy` defaults and `ai_provider_invoke_allows_loopback_for_trusted_workspace`) |
| Batch | Active | Batch payloads exist; trusted workspaces keep runtime apply enabled, untrusted workspaces default `runtime_apply_disabled = true` | `crates/legion-protocol/src/lib.rs` (`ProposalPayload::Batch`, `BatchProposalPayload`), `plans/evidence/phase-7/ws07-t1-apply-activation-audit.md`, `crates/legion-app/tests/workspace_vfs_integration.rs` (batch preflight and execution contract tests) |
| Plugin | Policy-gated / not productized as a write authority | Plugin host-call policy exists, namespace and capability checks exist, network and ambient host authority are denied by default, and plugin-produced workspace edits remain denied until activation gates exist | `crates/legion-security/src/lib.rs` (`PluginCapabilityPolicy` defaults, `plugin_network_process_filesystem_and_untrusted_workspace_are_denied`), `crates/legion-app/src/lib.rs` (`proposal.plugin_source_denied`), `docs/USER_GUIDE.md` |
| Remote | Policy-gated / not productized as a write authority | Remote sessions, filesystem, execution, LSP, semantic query, audit export, agent-package activation, and offline resume are disabled by default | `crates/legion-security/src/lib.rs` (`RemoteDevelopmentPolicy` defaults, `remote_capabilities_are_disabled_by_default_and_require_trust`, `remote_policy_allows_filesystem_without_execution`, `phase8_remote_egress_requires_runtime_session`), `docs/USER_GUIDE.md` |
| Collaboration | Policy-gated / not productized as a write authority | Collaboration sessions and mutation paths are disabled by default; presence can be enabled separately without enabling runtime mutation | `crates/legion-security/src/lib.rs` (`CollaborationCapabilityPolicy` defaults, `collaboration_capabilities_are_disabled_by_default_and_require_trust`, `collaboration_policy_allows_presence_without_runtime_mutation`), `docs/USER_GUIDE.md` |

## Summary

- The mutation routes that currently operate as product paths are manual save, text edit, closed-file mutation, workspace edit, LSP action lowering, AI proposal generation, and batch apply planning.
- Plugin, remote, and collaboration are present as policy-encoded surfaces but remain disabled or non-productized by default; they are not treated as open write authorities.
- No known mutation route in the current backlog list is missing from this inventory.

## Verification commands

- `cargo test -p legion-app`
- `cargo test -p legion-security`
