# Ticket 63 native named HTTP MCP settings delivery

Worktree: `D:/legion-ide-2026-workers/wave1-mcp-settings`, clean base `e671d8c`.
Ticket 63 remains open; this evidence does not accept or close it.

## Boundaries and planned checks

This slice renders named HTTP client configuration, explicit selection, retained
health, route/revision-bound credential entry, network grant, explicit connection
and health probe, and local grant/credential revocation. It reuses AppComposition,
the existing SecretStore port, HTTP client, and DesktopSessionStore. No inference
provider is required. Manual continues to deny activation/probing, including
loopback. Restore must never select, grant or connect a peer automatically.
Client stdio and server activation remain explicitly unsupported in settings.

Shared desktop edits are confined to MCP action enum/bridge arms, runtime MCP
settings dispatch and selection, and view module/state/projection/settings wiring.
Provider forms, provider configuration, editor/save composition and shared program
indices/status counts are outside this lane. Demonstrated settings/async gaps are
implemented in `named_mcp_peer.rs`. Root additionally authorized two minimal shared
`app/lib.rs` policy/mode hunks, described below; no shared state was added.

Approved public seams: desktop frame input/accessibility projection and action
dispatch, AppComposition inspect/capture/restore, injected synthetic SecretStore,
existing session store reopened from real files, controlled loopback HTTP
transcripts. Headless input is component evidence, not native window qualification.
No personal credentials, independently operated remote endpoints or GUI launch.

Planned checks use `cargo -j 2 --target-dir target` in this worktree: new desktop
settings behavior tests in individual red/green cycles, local MCP protocol and
persistence regression coverage, targeted desktop clippy, offline compilation,
scoped rustfmt and diff whitespace checks. Successful checks are not repeated.
Free D: space before compilation: approximately 1.30 TB.

## Product path and drain authority

Settings -> MCP Peers adds/edits named HTTP client metadata, explicitly selects a
peer, inspects local health, grants its exact transport revision, connects/probes,
and revokes the local grant or route-bound credential. Metadata preflight uses the
existing strict persistence codec before mutating app state. Session restoration
does not select, grant, load credentials or connect. Masked token entry is bound to
the displayed identity/revision, zeroizes through existing SensitiveString, clears
undo state, and clears on selection, route changes, settings exit or submission.
Credential writes/deletes use the injected existing SecretStore. Store/transport
errors are typed/redacted. An invalid metadata form retains input for correction.

The synchronous existing HTTP client was demonstrated to block a native headless
input frame for **1.0210083 seconds** behind a controlled delayed initialize reply
(`target/ticket63-settings-slow-red.log`). Settings now invokes app-owned start
methods and polls finished workers. No desktop thread owns an AppComposition
mutator; it emits typed decisions. Existing HTTP request timeouts remain in the
transport. No elapsed timer is interpreted as cancellation completion.

App workers retain the existing `named_provider_snapshots` atomic drain lease
until actual network/keyring work returns. The existing bounded MCP permit/reaper
retains retired handles; replacing configuration or dropping the app invalidates
the old transport gate without joining live HTTP or releasing its lease early.
Results require the current identity/revision/live grant and current policy.

The first stronger Manual and signed-ceiling regressions both failed because the
visible mode became Manual while the HTTP oracle still held its response. The
fix revokes pending gates when Manual is *requested*, then lets the existing mode
setter defer the visible transition on the common drain counter. A requested
Manual switch must be retried after drain on this baseline; a lowered installed
ceiling is reconciled by app polling after actual drain. Retained Assist/Delegate
mode never admits new MCP I/O above that ceiling: the existing ceiling predicate
is checked on synchronous activation, async preflight and runtime reuse. A late
revoked completion cannot publish Ready or send the next protocol message.

### Exact shared composition hunks

- `AppComposition::set_org_policy_bundle`: after retaining the verified bundle,
  call new private `install_mode_policy_ceiling` rather than assign Manual.
- `install_mode_policy_ceiling`: if the installed ceiling denies the current mode,
  call the existing `set_product_mode(Manual)` and its common live-lane guards.
- `AppComposition::set_product_mode`: an AI-enabled non-Assist target first calls
  MCP-only `request_named_mcp_operation_drain`, before the pre-existing snapshot
  count check. No initializer, save method or delegated-admission helper changed.

`ee6896f1e340113cfa1cddbd873f2bc7cf0cb8e7` was read via `git show` as a reference;
it was not merged over this worktree. Its `set_org_policy_bundle` has provider
check cancellation/deferred downgrade code in the same hunk. Pauli must preserve
that cancellation, compose it with the common setter/lease guard above, and make
provider and MCP completion reconciliation use the same drain-aware path. This
standalone patch does not duplicate or claim to qualify Mendel's delegated-task
admission fix. Combined concurrent provider+MCP qualification belongs to root and
Pauli, including a provider completing first while retired MCP HTTP remains held.

## Targeted behavioral verification

All logs below are in this worktree's ignored `target/`; no previously successful
settings case was repeated without a behavior-changing edit. Exact command prefix
for Rust tests: `cargo test -j 2 --target-dir target`.

| Check | Result | Log |
| --- | --- | --- |
| Native metadata input, real session-file reopen, no auto-connect/select | 1 passed | `ticket63-settings-green1-repair1.log` |
| Grant/non-Manual/protocol/health/revoke, adjusted for async completion | 1 passed in 3-test run | `ticket63-settings-remaining-contracts.log` |
| Masked route-bound token, wire authorization, secure deletion | 1 passed; later isolated diagnostic also passed | `ticket63-settings-green3-repair2.log`, `ticket63-settings-async-abort-diagnostic.log` |
| Slow connect responsiveness and revoke rejecting late completion | red: blocked 1.0210083s; green: 1 passed | `ticket63-settings-slow-red.log`, `ticket63-settings-slow-green-repair1.log` |
| Edit retires grant; stale/userinfo/oversize preflight preserves state/file | 1 passed | `ticket63-settings-edit-green.log` |
| Route/settings changes clear drafts; store failures redact and revoke | 1 passed in 3-test run | `ticket63-settings-remaining-contracts.log` |
| Retired-worker/app-drop responsiveness and old-result rejection | 1 passed initially, then affected stronger lease regression passed | `ticket63-settings-remaining-contracts.log`, `mcp-settings-honest-drain-green.log` |
| Invalid metadata form retains editable input | red then 1 passed | `ticket63-settings-draft-red.log`, `ticket63-settings-draft-green.log` |
| Slow health, honest Manual deferral, no late Ready, Manual accepted after drain | red: false Manual; green: passed in affected 2-test run | `mcp-settings-honest-drain-red.log`, `mcp-settings-honest-drain-green.log` |
| Public app signed lower ceiling denies async/sync new dispatch and waits for active **and retired** HTTP | red: false Manual; green: 1 passed, both retirement branches; strengthened non-vacuity control passed | `mcp-settings-ceiling-drain-red.log`, `mcp-settings-ceiling-drain-green.log`, `mcp-settings-ceiling-drain-control.log` |

The new desktop file has **9 distinct tests**, all with focused passing runs; the
new app file has **1 distinct test with 2 active/retired branches**, passing after
the shared hook repair. This is not a claim of one uninterrupted green suite run.
Session 51100's red app test was collected before editing; it was compiling a
distinct dependency graph, not hung. No known running test session was discarded.
No unresolved behavioral repair reached two different failed repair attempts.
The egui credential-label compile repair needed a second supported revision and
then passed. Final scoped gate results and file hashes are recorded below.

The signed-ceiling test was strengthened after inspection showed Unknown trust
could independently deny network admission. It now first connects successfully
under a signed Assist-ceiling bundle explicitly allowing its loopback test route,
then changes **only** that bundle's mode ceiling to Manual. Both active and retired
branches pass. This justified rerunning only that changed test; no successful
desktop settings cases were repeated at this stage.

## Final scoped gates and handoff

| Gate | Result | Log in `target/` |
| --- | --- | --- |
| `cargo clippy -j 2 --target-dir target -p legion-desktop --lib --test mcp_settings -- -D warnings` | Passed after one supported local `.and_then` -> `.and` lint repair | `mcp-settings-clippy.log`, `mcp-settings-clippy-repair1.log` |
| `cargo check -j 2 --target-dir target -p legion-desktop --no-default-features --features offline --lib` | Passed | `mcp-settings-offline-check.log` |
| Existing app MCP protocol/binding/denial tests | 4 passed | `mcp-settings-existing-app-regressions.log` |
| Existing signed mode-ceiling tests | 5 passed | `mcp-settings-existing-app-regressions.log` |
| `cargo test -j 2 --target-dir target -p legion-desktop --test named_mcp_peer_persistence` | 6 passed | `mcp-settings-persistence.log` |
| Scoped rustfmt, eight changed Rust files, edition 2024, `skip_children=true` | Seven initially clean; only workflow action wrapping required an affected-file repair; changed app test also formatted/checked | `mcp-settings-format-check.log`, `mcp-settings-format-repair.log`, `mcp-settings-ceiling-control-format.log` |

Existing app regression command (run once):

```powershell
cargo test -j 2 --target-dir target -p legion-app --test named_mcp_peer --test org_policy_mode_ceiling -- --exact named_http_peer_negotiates_pinned_protocol_and_scoped_auth_without_inference activation_requires_nonmanual_mode_current_transport_grant_and_peer_credential changing_endpoint_or_scopes_cannot_send_a_previous_binding_bearer negotiation_mismatch_malformed_health_and_redirect_disable_runtime_without_fallback a_mode_above_the_ceiling_is_refused a_mode_at_or_below_the_ceiling_is_permitted installing_a_bundle_lowers_a_mode_that_is_already_above_the_ceiling without_a_bundle_no_ceiling_applies the_ceiling_predicate_matches_the_bundle --test-threads=1
```

Final affected desktop command (2 passed):

```powershell
cargo test -j 2 --target-dir target -p legion-desktop --test mcp_settings -- --exact slow_health_probe_keeps_mode_honest_until_transport_drains_and_rejects_late_ready reconfiguration_and_app_drop_retire_slow_workers_without_blocking_or_publishing_old_health --test-threads=1
```

Final changed app command (1 passed with both active/retired branches):

```powershell
cargo test -j 2 --target-dir target -p legion-app --test named_mcp_settings_drain
```

**25 distinct tests have focused passing evidence:** 9 new desktop + 1 new app +
4 existing app MCP + 5 existing mode ceiling + 6 existing desktop persistence.
The historical abnormal run below is explicitly not included as a passing run.
All known cargo sessions were collected. The offline check reports 42 warnings
in existing app offline-gated imports/dead code; the vendored epaint f32 fallback
warning also remains. No unrelated warning cleanup was performed.

Changed paths (9 total; no provider/save files or shared status/index changes):

- `crates/legion-app/src/lib.rs` (two shared policy/mode hunks, 12 added/7 removed)
- `crates/legion-app/src/named_mcp_peer.rs`
- `crates/legion-app/tests/named_mcp_settings_drain.rs` (new)
- `crates/legion-desktop/src/bridge.rs` (four MCP action variants/exhaustive bridge arms)
- `crates/legion-desktop/src/view.rs` (MCP module/state/settings section/projection only)
- `crates/legion-desktop/src/workflow.rs` (MCP selection/actions/polling/projection only)
- `crates/legion-desktop/src/view/mcp_settings.rs` (new)
- `crates/legion-desktop/tests/mcp_settings.rs` (new)
- `plans/evidence/ide-2026-ticket063-mcp-settings.md` (this file)

Base: `e671d8c62e012646e5632521ae78c6970a987f9c`. Handoff artifact paths in this
worktree: `target/ticket063-mcp-settings.patch` and
`target/ticket063-mcp-settings-handoff.json`. The JSON records SHA-256 and byte
counts for all nine changed files, the patch and each retained test/gate log.
Patch creation does not stage or commit files. `git diff --check` and the patch's
read-only `git apply --reverse --check` validate whitespace/current contents;
their receipt is `target/mcp-settings-handoff-check.log`.

Pauli must explicitly compose the shared mode/policy hunk with provider patch40
and the final admission helpers, then run the combined provider+MCP slow-drain
regression. Native GUI/OS keyring, live peers, cross-platform and full product
gates were not run. This is a stable component handoff, not ticket acceptance.

## Retained concerns

The prior core checkpoint's unexplained Windows `0xc0000409` abnormal abort is
unresolved. It recurred in this lane's affected async credential/protocol run:
`ticket63-settings-async-affected.log` records STATUS_STACK_BUFFER_OVERRUN before
the credential test completed. The isolated credential diagnostic passed without
a product repair; that pass does not repair or invalidate either observation.

Exact recurrence command, from this worktree:

```powershell
cargo test -j 2 --target-dir target -p legion-desktop --test mcp_settings -- --exact selected_peer_requires_grant_and_nonmanual_mode_before_protocol_health_and_revocation masked_peer_credential_is_bound_to_reviewed_route_used_only_after_grant_and_deleted_on_revoke --test-threads=1 *> target/ticket63-settings-async-affected.log
```

Absolute log path:
`D:/legion-ide-2026-workers/wave1-mcp-settings/target/ticket63-settings-async-affected.log`.
Surrounding retained output:

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 6.09s
Running tests\mcp_settings.rs (target\debug\deps\mcp_settings-5893e74b275e7005.exe)
running 2 tests
error: test failed, to rerun pass `-p legion-desktop --test mcp_settings`
Caused by:
process didn't exit successfully ... (exit code: 0xc0000409, STATUS_STACK_BUFFER_OVERRUN)
note: test exited abnormally; to see the full output pass --no-capture to the harness.
test masked_peer_credential_is_bound_to_reviewed_route_used_only_after_grant_and_deleted_on_revoke ...
```

The only test-start marker in that log is the masked credential test; it has no
`ok`/completion marker. The selected-peer test has no start or completion marker.
**Neither test completion is evidenced by this aborted command.** The subsequent
isolated credential pass (`ticket63-settings-async-abort-diagnostic.log`, with
RUST_BACKTRACE=1/--nocapture) is diagnostic evidence, not a repair. The selected
case subsequently passed in the behavior-adjusted three-test run. No debugger
dump or stack artifact was captured; no .dmp/.mdmp/.hdmp was found in this
worktree's target directory. No cause is inferred from the exit status or from
the separate test-fixture WouldBlock repair.

Native OS keyring/input, real peers (tickets 64/65), stdio/server containment,
concurrent cancellation/recovery (ticket 66), cross-platform and product/release
qualification remain separate gates. Root/Pauli own aggregation and integration;
Pauli is the sole committer. This lane performs no commit, push, PR or merge.

## Independent review and local composition disposition

Pauli's final independent review is PASS for bounded local partial implementation,
with the unresolved runtime concern below retained. All 45 entries in the frozen
worker manifest were verified before composition. The worker patch SHA-256 is
`4f86b9073081ce49e3aa54ce25f7feac84a190127aa0ec7f19d2cb19ee686fa5`.
This integration appendix was added after that frozen handoff; the manifest's
receipt hash identifies the worker version, not this appended version.

The patch was composed on provider source
`ee6896f1e340113cfa1cddbd873f2bc7cf0cb8e7`. Provider cancellation/admission and
ticket 05's retained-buffer save baseline remain preserved. The coordinator
independently reviewed Pauli's two production composition hunks and the new
held-HTTP regression: PASS. The production hashes remained unchanged afterward.
The new regression passed all four active/retired MCP and response-order branches;
default/offline desktop compilation and dependency/canvas gates passed.
[Integration evidence](ide-2026-provider-mcp-integration.md) records exact commands,
logs, hashes, scope and the separate final documentation/build steps.

The authorized debugger diagnosis retained a first, inconclusive capture-harness
failure: startup was terminated before test output, so it establishes no product
test pass or failure. The final corrected synchronous LLDB invocation reused the
frozen EXE/PDB and completed the exact two selected tests, both passing with
inferior exit 0. The abort was not reproduced in this one completed diagnostic.
No code repair was made and neither historical abort is explained or erased.
Receipt: `D:/legion-ide-2026-notes/ticket063-lldb-20261009-162447-e096a38e/receipt.md`.

Root authorized the reviewed local partial implementation after that disposition
and the composed regression PASS. The intermittent Windows abnormal abort remains
an explicit native/product/release qualification blocker. Ticket 63 remains open;
there is no green-suite, native settings, live-peer or release acceptance claim.
