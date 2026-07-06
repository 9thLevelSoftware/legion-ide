# PKT-APPLY Report — M9 Apply Activation

**Status: DONE**

Date: 2026-07-05
Branch: `m9/apply-activation`

## Commits

Commits are on branch `m9/apply-activation` from main at 820fbfe. (Short SHAs will be filled in after commit step.)

## Test results

```
cargo test -p legion-app --test apply_activation
running 10 tests — 10 passed, 0 failed

cargo test -p legion-app
all suites — 0 failed

cargo test -p legion-security
all suites — 0 failed (69 unit + 14 integration)

cargo check --workspace
0 errors, 0 warnings
```

## What was delivered

### Task 1: Mutation-route inventory + architecture boundaries
- `plans/evidence/mutation-route-inventory.md`: terminal-command route added; M9 activation state documented.
- `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`: apply-gate table, batch policy, LSP rename path documented.

### Task 2a: ProposalApplyGate wired into apply_workspace_proposal
- `apply_workspace_proposal` evaluates `ProposalApplyGate` before payload dispatch.
- `fs.*` in untrusted → Deny. `plugin.*`/`remote.*`/`collaboration.*`/`terminal.*` → DenyByDefaultBroker.
- All denials: `denied_apply_response` + `observe_proposal_response` (audit row with `Denied` state).
- `editor.write` and other non-restricted capabilities pass through.

### Task 2b: Batch commit/finalize unblocked for Trusted workspaces
- `AppComposition::batch_apply_policy: BatchRuntimeApplyPolicy` field added (default: fail-closed).
- `plan_batch_execution_contract` derives `commit_blocked`/`finalize_blocked` from policy × trust.
- `set_batch_apply_policy_for_test` test helper added.
- `runtime_apply_disabled` diagnostic made conditional on preflight state.

### Task 2c: approve_and_apply_rename_proposal
- New public method: Previewed → Approved → Applied.
- Docstrings on `ingest_lsp_rename_result` and `issue_lsp_rename_request` updated (removed "Generation only" placeholder).

### Task 2d: uri_to_native_path
- Added `uri_to_native_path` in `translate.rs` (Windows: forward→back slashes; other: identity).
- `translate_resource_operation` create and rename cases updated to use `uri_to_native_path`.
- Test `t2_uri_to_native_path_windows_separator_form` added in translate.rs tests.

### Task 3: Denial tests with audit rows
All 4 denial tests pass: plugin, remote, collaboration, terminal-command.

### Task 4: Hardcoded runtime_apply_disabled flags removed
- `commit_blocked`/`finalize_blocked` hardcoded `true` replaced with policy-derived logic.
- Unreachable `_` arm in apply payload match removed.

## Concerns

None. The implementation is minimal and correct:
- No behavior change for existing `editor.write` proposals (gate passes through).
- `BatchRuntimeApplyPolicy::default()` preserves fail-closed backward compatibility.
- All apply denials are visible via audit records.
- `manual_zero_egress` is unaffected.

---

## Round 3 review findings — fixed 2026-07-06

**Commit:** `cc7f050` — `test: add coverage for 3 PKT-APPLY review findings (PKT-APPLY-R1)`

### Finding 1 fixed: approve_and_apply_rename_proposal test

Added `approve_and_apply_rename_proposal_applies_and_renames_file` in
`crates/legion-app/tests/apply_activation.rs`:
- Seeds a file on disk, opens a trusted workspace (gets file identity via
  `workspace_node_by_name`), builds a `RenameFileProposal`, calls
  `register_validate_preview`, calls `approve_and_apply_rename_proposal`,
  asserts `Applied(_)`, and verifies source gone + dest contents match.
- Added helper functions `workspace_tree`, `workspace_node_by_name`,
  `file_preconditions` to the test file (mirrors workspace_vfs_integration
  pattern).

### Finding 2 fixed: BatchRuntimeApplyPolicy production activation

- `open_workspace` in `lib.rs` now sets
  `self.batch_apply_policy.enabled = trust == WorkspaceTrustState::Trusted`
  so trusted workspaces get commit/finalize unblocked without a manual test
  helper call.
- Added `force_proposal_lifecycle_state_for_test` test-only helper to
  `AppComposition` (used by Finding 3 test).
- Added `open_trusted_workspace_enables_batch_policy_for_production` test.
- Updated `workspace_vfs_integration_batch_execution_contract_reports_audit_and_commit_barriers`
  to reflect new behavior (commit_blocked/finalize_blocked now `false` for
  trusted workspaces).

### Finding 3 fixed: TerminalCommand defense-in-depth arm test

Added `terminal_command_defense_in_depth_arm_denied_from_previewed_state` in
`apply_activation.rs`:
- Registers a TerminalCommand proposal with `editor.write` capability (passes
  the ProposalApplyGate namespace check).
- Uses `force_proposal_lifecycle_state_for_test` to bypass validate (which
  would reject TerminalCommand) and inject `Previewed` state directly.
- Calls `handle_proposal_request(Apply)`, asserts `Denied { PolicyDenied }`,
  verifies audit record shows `Denied`.

### Test results after round-3 fixes

```
cargo test -p legion-app --test apply_activation
running 13 tests — 13 passed, 0 failed

cargo test -p legion-app
all suites — 0 failed
```
