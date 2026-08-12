# P0 Governance Mutation-Path Audit

Status: P0.1/P0.2 implementation evidence  
Prepared: 2026-05-21  
Scope: mutation-path classification and non-regression scaffolding for the current control-first envelope.

## Control-first invariants audited

- Durable saves remain proposal-mediated through [`AppComposition::save_active_buffer()`](../../crates/legion-app/src/lib.rs), [`SaveWorkflowService`](../../crates/legion-app/src/lib.rs), and [`WorkspaceActor::save_file_with_proposal()`](../../crates/legion-project/src/lib.rs).
- UI remains projection-only: [`legion-ui`](../../crates/legion-ui/src/lib.rs) consumes protocol projections, emits [`CommandDispatchIntent`](../../crates/legion-ui/src/ui.rs), and has no direct dependency on editor, project, storage, or app crates.
- Non-save generalized proposals remain fail-closed unless an accepted phase gate adds execution evidence. P0 does not implement Phase P1 proposal execution.
- Observability and proposal audit records remain metadata-only; this audit does not authorize source, prompt, secret, terminal transcript, or provider payload persistence.
- ADR-0015 streaming constraints remain in force: full-source UI projection is limited to explicitly bounded small-buffer mode, and large/degraded buffers use viewport projections.

## Permitted mutation paths

| Class | Permitted entry points | Authority owner | Current status | P0 guard |
| --- | --- | --- | --- | --- |
| Direct user edits | UI input is converted to [`CommandDispatchIntent`](../../crates/legion-ui/src/ui.rs), routed by app command dispatch, and applied through editor-owned transaction APIs. | App command routing + editor engine | Allowed for active user editing only; not durable disk mutation. | [`legion-ui`](../../crates/legion-ui/Cargo.toml) depends only on [`legion-protocol`](../../crates/legion-protocol/src/lib.rs); `xtask` projection-only assertions reject editor/project/storage/app coupling. |
| Save proposals | Manual save builds a save proposal, validates/previews it, and calls workspace save with mandatory preconditions. | [`SaveWorkflowService`](../../crates/legion-app/src/lib.rs) + [`WorkspaceActor::save_file_with_proposal()`](../../crates/legion-project/src/lib.rs) | Allowed; stale, conflict, denial, and failed/rejected outcomes preserve dirty editor text. | App integration tests cover untrusted denial, oversized denial, external overwrite stale/conflict, and disappeared-file failure/conflict paths. |
| Generalized workspace proposals | Proposal DTOs, target coverage, preflight, validation, preview, lifecycle records, and batch contracts. | App proposal coordinator + protocol DTOs | Runtime apply beyond accepted save/open-buffer slices remains denied or preflight-only until later phase gates. | P0 makes no Phase P1 executor changes; existing unsupported execution diagnostics remain fail-closed. |
| Test-only helpers | Mock command ports, temporary roots, proposal payload builders, and in-memory sinks in integration/unit tests. | Test modules only | Allowed only to prove routing, denial, dirty-text preservation, metadata-only events, and static gates. | Helpers must not introduce production write paths or bypass save proposals in runtime code. |

## Dirty-text preservation coverage

The current save-path regression suite in [`workspace_vfs_integration.rs`](../../crates/legion-app/tests/workspace_vfs_integration.rs) covers:

- Denial: untrusted saves and oversized saves return rejected outcomes without disk mutation and keep the dirty buffer text.
- Stale/conflict: external overwrite between open and save returns a stale/conflict rejection, preserves external disk content, and keeps the dirty buffer text.
- Failure/conflict: disappeared-file save failure/conflict returns a rejected outcome and keeps the dirty buffer text.

## P0 implementation notes

- Dependency policy now records [`legion-ui`](../../crates/legion-ui/Cargo.toml) as protocol-only and explicitly forbids `legion-ui -> legion-app`, `legion-ui -> legion-editor`, `legion-ui -> legion-project`, and `legion-ui -> legion-storage`.
- UI command coordinates now use protocol text coordinate DTOs instead of editor text types, keeping editor coordinate conversion inside app command routing.
- `xtask` static checks were strengthened to assert the UI manifest does not depend on app/editor/project/storage crates and that the save path still contains proposal-mediated routing markers.

## Non-scope confirmation

No semantic index, LSP, AI, agent, plugin, terminal, remote, collaboration, placeholder-crate activation, or generalized proposal execution runtime behavior is implemented by this P0 slice.
