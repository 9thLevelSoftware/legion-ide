# PKT-APPLY Evidence — M9 Apply Activation

Date: 2026-07-05
Branch: `m9/apply-activation`

## Task summary

PKT-APPLY wires existing security policy types into the live apply path, unblocks batch execution for trusted workspaces, adds the LSP rename Approve→Apply path, and adds Windows-native path support for URI-derived apply-time path matching.

## What was implemented

### Task 2a — ProposalApplyGate wired into apply_workspace_proposal

`apply_workspace_proposal` (legion-app/src/lib.rs) now evaluates a `ProposalApplyGate` before the payload dispatch:

- `fs.*` capabilities: allowed in Trusted workspaces; denied in Untrusted/Unknown with `proposal.apply_gate_denied` audit code.
- `plugin.*`, `remote.*`, `collaboration.*`, `terminal.*`: routed through `DenyByDefaultBroker::default().decide()` — denied by default.
- All other capabilities (e.g. `editor.write`): allowed through (lifecycle/workspace checks are the authority).

On denial, `denied_apply_response` is called (records `ProposalLifecycleState::Denied`) and `observe_proposal_response` persists the audit row.

### Task 2b — BatchRuntimeApplyPolicy unblocks batch commit/finalize for trusted workspaces

`AppComposition` now holds a `batch_apply_policy: BatchRuntimeApplyPolicy` field (default: fail-closed, `enabled: false`). `plan_batch_execution_contract` derives `commit_blocked` and `finalize_blocked` from the policy and workspace trust:

- Default policy → always blocked (backward-compatible).
- `enabled: true` + Trusted workspace → `commit_blocked = false`, `finalize_blocked = false`.
- `enabled: true` + Untrusted workspace → still blocked.

Test helper: `set_batch_apply_policy_for_test` (behind `cfg(any(test, feature = "test-helpers"))`).

### Task 2c — approve_and_apply_rename_proposal (Previewed→Approved→Applied)

New public method `approve_and_apply_rename_proposal(proposal_id)` on `AppComposition`:

1. If the proposal is in `Previewed` state, issues a `ProposalLifecycleCommand::Approve` transition.
2. Re-fetches the proposal after approval.
3. Calls `apply_workspace_proposal` with the now-Approved proposal.

The existing `ingest_lsp_rename_result` and `issue_lsp_rename_request` docstrings were updated to reference this method (removed "Generation only" / "Application is P3.F1.T2/M9 scope" placeholders).

### Task 2d — uri_to_native_path in translate.rs

`uri_to_native_path` added after `uri_to_canonical_path` in `crates/legion-app/src/language/translate.rs`:
- On Windows: converts forward slashes to backslashes.
- On other platforms: identity function.

Apply-time call sites in `translate_resource_operation` (create and rename cases) updated to use `uri_to_native_path`.

### Task 3 — Plugin/remote/collaboration/terminal-command denial tests

New tests in `crates/legion-app/tests/apply_activation.rs`:

| Test | What it proves |
| --- | --- |
| `plugin_source_proposal_apply_is_denied_with_audit` | `plugin.fs` capability → Denied with audit row |
| `remote_source_proposal_apply_is_denied_with_audit` | `remote.session.connect` → Denied with audit row |
| `collaboration_source_proposal_apply_is_denied_with_audit` | `collaboration.session.create` → Denied with audit row |
| `terminal_command_proposal_apply_is_denied_with_audit` | `TerminalCommand` payload → rejected at validate (Unsupported/Denied), never Applied, audit row recorded |

### Task 4 — Hardcoded runtime_apply_disabled flags removed

The hardcoded `commit_blocked: true, finalize_blocked: true` at the previous `plan_batch_execution_contract` have been replaced by policy-derived values (Task 2b). The `runtime_apply_disabled` diagnostic is now conditional on `preflight.runtime_apply_disabled` being true.

### Task 1 — Mutation-route inventory and architecture boundaries updated

- `plans/evidence/mutation-route-inventory.md`: added `terminal-command` route row; added M9 PKT-APPLY activation state section.
- `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`: added M9 apply-gate activation table.

## Test results

```
cargo test -p legion-app --test apply_activation
running 10 tests
test trusted_workspace_with_enabled_policy_unblocks_batch_commit_finalize ... ok
test untrusted_workspace_keeps_batch_commit_finalize_blocked ... ok
test terminal_command_proposal_apply_is_denied_with_audit ... ok
test collaboration_source_proposal_apply_is_denied_with_audit ... ok
test remote_source_proposal_apply_is_denied_with_audit ... ok
test plugin_source_proposal_apply_is_denied_with_audit ... ok
test untrusted_workspace_proposal_is_denied_with_audit_row ... ok
test trusted_workspace_keeps_batch_runtime_apply_enabled ... ok
test untrusted_workspace_disables_batch_runtime_apply ... ok
test trusted_workspace_proposal_passes_apply_gate ... ok
test result: ok. 10 passed; 0 failed

cargo test -p legion-app
test result: ok. (all suites) 0 failed

cargo test -p legion-security
test result: ok. (all suites) 0 failed

cargo check --workspace
Finished dev profile — 0 errors
```

## Key files modified

| File | Change |
| --- | --- |
| `crates/legion-app/src/lib.rs` | ProposalApplyGate wired; batch_apply_policy field; plan_batch_execution_contract policy-derived; approve_and_apply_rename_proposal; docstrings updated |
| `crates/legion-app/src/language/translate.rs` | uri_to_native_path added; apply call sites updated |
| `crates/legion-app/tests/apply_activation.rs` | 8 new tests (gate denial, batch unblock, trusted/untrusted) |
| `plans/evidence/mutation-route-inventory.md` | terminal-command route added; M9 activation state |
| `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md` | M9 apply-gate activation table |

## Concerns

None. All security constraints satisfied:
- `manual_zero_egress` not touched.
- No stubs or fixture-only product paths.
- Default-deny preserved: `BatchRuntimeApplyPolicy::default()` is fail-closed.
- All apply denials produce audit rows via `observe_proposal_response`.
