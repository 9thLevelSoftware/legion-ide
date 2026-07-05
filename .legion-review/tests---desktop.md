# Review: Tests - Desktop

Scope: `crates/legion-desktop/tests` integration and workflow tests listed in kanban task `t_dcc8aa1f`.

Verification performed:
- Read all 39 assigned files.
- Scanned for TODO/FIXME/HACK, unimplemented stubs, panic/unwrap hotspots, source-inspection tests, and weak assertion patterns.
- Ran `cargo test -p legion-desktop --tests --no-run` successfully.
- Ran `cargo test -p legion-desktop --tests` successfully: all legion-desktop unit/integration tests passed.

Summary:
- Findings: 18
- Critical: 0
- High: 0
- Medium: 15
- Low: 3

## crates/legion-desktop/tests/accessibility.rs

No issues found in this file. The tests use projection-level fixtures and one headless input path with direct assertions over serialized accessibility profile fields and projected accessibility nodes.

## crates/legion-desktop/tests/agent_comm.rs

No issues found in this file. The parser test is small but directly asserts the one-row filtered result and all parsed fields.

## crates/legion-desktop/tests/assist_inline_prediction_workflow.rs

No issues found in this file. It verifies assist-mode gating, active ghost text projection, and clearing behavior after returning to Manual mode.

## crates/legion-desktop/tests/assistant_rail.rs

No issues found in this file. The test directly checks text/code/text segmentation and proposal affordance availability.

## crates/legion-desktop/tests/beta_acceptance_e2e.rs

### Finding 1
- File path: `crates/legion-desktop/tests/beta_acceptance_e2e.rs`
- Line number(s): 280-364
- Category: failure-point
- Severity: medium
- Description: The acceptance test comments claim coverage for approved VSIX install, context manifest inspection, proposal diff review, and test execution, but these sections instantiate local fixture structs (`approved_vsix_manifest`, `proposal_context_manifest_fixture`, `proposal_diff_fixture`, `test_verification_run_fixture`) rather than driving the real desktop/runtime/plugin/test surfaces. This can pass even if the actual VSIX installation bridge, proposal inspector, diff renderer, or verification-run workflow is broken.
- Suggested fix direction: Split fixture shape tests from real acceptance coverage. Add workflow-level actions that load/validate a fixture extension manifest through the plugin path, open proposal evidence via the bridge/view model, and record a verification run through the app/runtime path before asserting projection output.

### Finding 2
- File path: `crates/legion-desktop/tests/beta_acceptance_e2e.rs`
- Line number(s): 241-247, 520-526
- Category: failure-point
- Severity: medium
- Description: The no-bypass checks only assert that `report.proposal_status` does not contain the substrings `apply` or `autonomous`. A human-readable status string can be renamed and still hide an autonomous-apply capability elsewhere, or it can legitimately contain those words in a denial message and create a false failure.
- Suggested fix direction: Assert structured policy/proposal fields instead of status text. For example, assert lifecycle state, allowed action set, approval gate state, and absence of an autonomous-apply command in the projected command/action model.

## crates/legion-desktop/tests/beta_workflow.rs

### Finding 3
- File path: `crates/legion-desktop/tests/beta_workflow.rs`
- Line number(s): 90-100, 117, 139-142
- Category: failure-point
- Severity: medium
- Description: Several important beta workflow assertions rely on substring checks against status/error/evidence text (`contains("saved")`, `contains("denied")`, `contains("blocked")`, etc.). This is brittle and may miss regressions where the structured workflow state changes but the words remain, or fail on harmless copy changes.
- Suggested fix direction: Prefer structured fields/enums in `BetaWorkflowReport` and typed error variants. Keep text assertions only for markdown formatting coverage after the structured state has already been verified.

## crates/legion-desktop/tests/breakpoint_hit.rs

### Finding 4
- File path: `crates/legion-desktop/tests/breakpoint_hit.rs`
- Line number(s): 49, 81-91, 100-125, 132-153
- Category: failure-point
- Severity: medium
- Description: The test creates a Rust crate but then calls `enable_debug_fixture_for_tests()` and asserts fixture-projected paused state, rows, locals, and console entries. This does not exercise the actual DAP/debug-adapter launch, breakpoint binding, or locals retrieval path; a broken adapter integration could still pass.
- Suggested fix direction: Keep the fixture test for deterministic UI projection, but add a separate gated integration test that uses the real debug adapter/cargo launch path (or a protocol-level fake DAP server) and verifies breakpoint binding, stopped event, stack frame, and variables from protocol events.

## crates/legion-desktop/tests/collaboration_gui.rs

### Finding 5
- File path: `crates/legion-desktop/tests/collaboration_gui.rs`
- Line number(s): 345-375
- Category: failure-point
- Severity: medium
- Description: The workflow test enables a local collaboration fixture runtime and verifies join/presence status transitions, but it does not exercise a real collaboration transport, remote peer update, conflict payload, or shared proposal exchange. The nearby row test uses a fully synthetic projection, so network/session fidelity is not covered.
- Suggested fix direction: Add a transport-level fake or loopback collaboration runtime test that feeds remote presence/conflict/proposal events through the production ingestion path and asserts projected rows plus local buffer immutability.

## crates/legion-desktop/tests/control_trust_bridge.rs

### Finding 6
- File path: `crates/legion-desktop/tests/control_trust_bridge.rs`
- Line number(s): 337-347
- Category: failure-point
- Severity: medium
- Description: The projection-boundary test reads `src/bridge.rs` and `src/view.rs` as text and asserts that selected type names are absent. Source substring checks are easy to evade accidentally with aliases, re-exports, qualified paths not in the deny list, generated modules, or comments; they can also fail on harmless documentation.
- Suggested fix direction: Replace source-text checks with an architectural lint, dependency graph assertion, or compile-time boundary test that denies forbidden crate/module dependencies. If source scans remain, centralize a deny-list tool that tokenizes Rust rather than matching raw strings.

## crates/legion-desktop/tests/control_trust_view.rs

No issues found in this file. The tests cover multiple control/trust projection rows and selected evidence fields with direct assertions.

## crates/legion-desktop/tests/daily_editing_controls.rs

No issues found in this file. The tests verify app-authority routing, dirty-close prompt state, save-all outcomes, cursor/selection/scroll dispatch, and explorer toggling.

## crates/legion-desktop/tests/debug_workflow.rs

### Finding 7
- File path: `crates/legion-desktop/tests/debug_workflow.rs`
- Line number(s): 49, 78-90, 91-121, 128-148
- Category: failure-point
- Severity: medium
- Description: Like `breakpoint_hit.rs`, this test writes a Cargo project but switches to `enable_debug_fixture_for_tests()` before refreshing configs, launching, stepping, and evaluating. It verifies deterministic fixture projection rather than the real DAP/session lifecycle.
- Suggested fix direction: Keep this as a projection smoke test and add a real or fake-DAP integration test that validates launch request, breakpoint request, stopped event, step request, evaluate request, and error propagation from the protocol layer.

## crates/legion-desktop/tests/delegated_task_command_center.rs

No issues found in this file. The tests cover bridge routing, unknown row denials, metadata-only inspection, and command-center projection rows.

## crates/legion-desktop/tests/desktop_workflow.rs

No issues found in this file. The tests assert real file editing/saving, external overwrite rejection, quit flag behavior, and replace/delete app-authority routing.

## crates/legion-desktop/tests/diagnostics_export.rs

No issues found in this file. The tests cover configured export paths, metadata-only diagnostics, raw-data opt-in behavior, and source payload redaction checks.

## crates/legion-desktop/tests/diagnostics_harness.rs

### Finding 8
- File path: `crates/legion-desktop/tests/diagnostics_harness.rs`
- Line number(s): 83-100, 117-124
- Category: failure-point
- Severity: medium
- Description: The diagnostics harness only tests a matching `buffer_id`/URI happy path and then a clear event for the same file. It does not cover mismatched URI vs buffer id, diagnostics for unopened files, malformed diagnostic ranges, or untrusted/raw payload redaction in LSP diagnostics.
- Suggested fix direction: Add negative-path cases that send diagnostics with a different URI/buffer, invalid ranges, missing fields, and sensitive message text. Assert that diagnostics are rejected or sanitized and that unrelated buffers are not cleared.

## crates/legion-desktop/tests/git_workflow.rs

No issues found in this file. The tests use real temporary git repositories and local bare remotes, and verify diff/hunk staging, worktree classification, save-triggered refresh, conflict resolution, push, and PR URL translation.

## crates/legion-desktop/tests/headless_input.rs

### Finding 9
- File path: `crates/legion-desktop/tests/headless_input.rs`
- Line number(s): 31-35
- Category: failure-point
- Severity: low
- Description: The global headless input serialization lock uses `lock().unwrap()`. If one test panics while holding the lock, the mutex is poisoned and every later headless-input test will panic on the lock acquisition rather than reporting its own behavior.
- Suggested fix direction: Recover from poisoned locks in the test guard (`unwrap_or_else(|poisoned| poisoned.into_inner())`) or use a small helper that reports the original poisoning but still allows independent tests to run.

## crates/legion-desktop/tests/input_conformance.rs

No issues found in this file. The tests cover focused/unfocused keyboard and text routes, selection, mouse non-mutation, clipboard, and IME payload preservation.

## crates/legion-desktop/tests/intent_bridge.rs

### Finding 10
- File path: `crates/legion-desktop/tests/intent_bridge.rs`
- Line number(s): 1021-1027
- Category: failure-point
- Severity: medium
- Description: The app-boundary test reads `src/bridge.rs` and checks for a few forbidden names as raw substrings. This is brittle and incomplete: aliases, qualified imports not named in the list, or new app-owned types can bypass the check, while comments can fail it.
- Suggested fix direction: Replace with dependency-level enforcement (cargo-deny/guppy/cargo metadata check) or a Rust-aware lint that forbids `legion_app`/app-authority symbols in the bridge module.

## crates/legion-desktop/tests/keyboard_nav.rs

No issues found in this file. The test exercises keyboard activation for product-mode switching through the headless app path.

## crates/legion-desktop/tests/language_terminal_view.rs

No issues found in this file. The tests cover language panel rows, structural search rows, language action dispatch, and terminal panel rendering/dispatch.

## crates/legion-desktop/tests/language_terminal_workflow.rs

### Finding 11
- File path: `crates/legion-desktop/tests/language_terminal_workflow.rs`
- Line number(s): 224-232
- Category: failure-point
- Severity: medium
- Description: The projection-only boundary check uses raw source substring checks for `AppComposition`, `legion_terminal`, and `TerminalRuntime<`. This can miss real forbidden dependencies through aliases/wrappers and can fail on harmless comments or documentation.
- Suggested fix direction: Enforce the boundary through dependency graph checks or Rust AST/token-based lints, and keep workflow tests focused on observable bridge outputs and projection state.

## crates/legion-desktop/tests/large_file_guardrails.rs

No issues found in this file. The tests exercise streaming/viewport projection and bounded search behavior against a >5 MiB fixture.

## crates/legion-desktop/tests/legion_workflow_command_center.rs

No issues found in this file. The tests cover approval queue rows, workflow rows, bridge action routing/denials, health counts, ready-state mediation, automate rows, and kill/permission actions.

## crates/legion-desktop/tests/operational_health.rs

No issues found in this file. The tests verify metadata-only health rows, safe default labels, unsupported-surface reporting, and source-payload redaction.

## crates/legion-desktop/tests/packaging.rs

No issues found in this file. The tests verify plan construction for debug/release, metadata-only manifest contents, and dry-run manifest output.

## crates/legion-desktop/tests/palette_coverage.rs

### Finding 12
- File path: `crates/legion-desktop/tests/palette_coverage.rs`
- Line number(s): 345-412
- Category: failure-point
- Severity: medium
- Description: The command palette coverage test increments `resolved_cases` over a local fixed `cases` array and divides by a hard-coded denominator of `13`. If the command catalog grows, this test still reports 100% coverage for the old curated list and does not fail for newly uncovered commands.
- Suggested fix direction: Derive the denominator and expected cases from the actual command catalog/registry, or assert that every catalog command appears in the test-case table. Avoid a hard-coded coverage denominator.

## crates/legion-desktop/tests/plan_editor.rs

No issues found in this file. The test directly verifies editable plan sections, summary labeling, and deterministic row output.

## crates/legion-desktop/tests/platform_integration.rs

No issues found in this file. The tests assert platform smoke snapshot fields and metadata-only accessibility labels.

## crates/legion-desktop/tests/platform_smoke.rs

### Finding 13
- File path: `crates/legion-desktop/tests/platform_smoke.rs`
- Line number(s): 137-162
- Category: failure-point
- Severity: low
- Description: `platform_smoke_report_writes_evidence_file` uses a temp directory name based only on the process id and cleans it up manually at the end. If the assertion path panics before cleanup, stale evidence can remain and later invocations in the same process id namespace can collide or read leftover state.
- Suggested fix direction: Use the same nanos/counter-based temp helper pattern as the other tests and a `Drop` guard for cleanup, or use `tempfile` if available.

### Finding 14
- File path: `crates/legion-desktop/tests/platform_smoke.rs`
- Line number(s): 229-233
- Category: failure-point
- Severity: medium
- Description: `platform_smoke_adapter_paths_route_without_metrics_payloads` reads `src/metrics.rs` and asserts that broad substrings like `String,` are absent. This is not tied to any metric payload API and can fail on unrelated legitimate code while still missing payload leakage via aliases or different field names.
- Suggested fix direction: Assert the actual exported metrics/report structs and serialized evidence schema. If payload fields must be forbidden, use a Rust-aware structural check or serialization snapshot rather than broad raw substring checks.

## crates/legion-desktop/tests/plugin_management.rs

### Finding 15
- File path: `crates/legion-desktop/tests/plugin_management.rs`
- Line number(s): 81-99, 223-253
- Category: failure-point
- Severity: low
- Description: The signed extension/grammar tests use a synthetic projection with a hard-coded `file:///tmp/rust-plugin-grammar.wasm` artifact URI and only assert rendered row strings. They do not verify artifact path canonicalization, trust-policy enforcement, hash validation, or rejection of unsafe local artifact URIs.
- Suggested fix direction: Add workflow/manifest-level tests that load signed extension metadata through the production plugin validation path and assert safe URI handling, hash checks, and denial of artifacts outside approved roots.

## crates/legion-desktop/tests/projection_rendering.rs

### Finding 16
- File path: `crates/legion-desktop/tests/projection_rendering.rs`
- Line number(s): 1716-1721
- Category: failure-point
- Severity: medium
- Description: The renderer app-boundary test reads `src/view.rs` and asserts that `legion_app` and `AppComposition` do not appear. This raw substring check is too narrow to enforce architectural boundaries and too broad for harmless comments/docs.
- Suggested fix direction: Enforce renderer dependency boundaries with cargo metadata or a Rust-aware lint, and reserve this test file for rendering/projection behavior.

## crates/legion-desktop/tests/remote_workspace_gui.rs

### Finding 17
- File path: `crates/legion-desktop/tests/remote_workspace_gui.rs`
- Line number(s): 339-355, 357-382
- Category: failure-point
- Severity: medium
- Description: The remote workflow test enables a local remote-development fixture and verifies a connected projection, but it does not exercise remote transport handshakes, offline/reconnect events, remote terminal/LSP descriptors from a backend, or proposal synchronization. A broken transport integration could pass while the projection fixture remains correct.
- Suggested fix direction: Add a loopback/fake remote backend test that drives connect, reconnect/offline, terminal descriptor, LSP descriptor, and remote proposal events through the production ingestion path before asserting GUI rows and local buffer immutability.

## crates/legion-desktop/tests/sandbox_panel.rs

No issues found in this file. The test checks active backend, caveats, and policy note rendering from sandbox panel rows.

## crates/legion-desktop/tests/save_all_conflict.rs

### Finding 18
- File path: `crates/legion-desktop/tests/save_all_conflict.rs`
- Line number(s): 274-277
- Category: failure-point
- Severity: medium
- Description: `save_all_conflict_desktop_save_paths_dispatch_ui_intent` verifies save-path architecture by scanning `workflow.rs` for `dispatch_ui_intent` and absence of `save_file_with_proposal`. This can miss equivalent bypasses under different helper names and can fail on harmless references or comments.
- Suggested fix direction: Replace with behavior-level tests that inject a spy/mock app authority and assert save/close/save-all commands always dispatch UI intents, or enforce forbidden workflow calls with a Rust-aware lint.

## crates/legion-desktop/tests/scope_picker.rs

No issues found in this file. The test verifies structured scope model fields and round-trip equality.

## crates/legion-desktop/tests/search_workflow.rs

No issues found in this file. The tests cover active-file search, workspace search, structural preview without mutation, no-results/validation errors, cancellation, and degraded projection display.

## crates/legion-desktop/tests/session_restore.rs

No issues found in this file. The tests cover session save/restore, missing-file skipped tabs, settings persistence, corrupt JSON errors, raw source marker rejection, and temp/backup cleanup.
