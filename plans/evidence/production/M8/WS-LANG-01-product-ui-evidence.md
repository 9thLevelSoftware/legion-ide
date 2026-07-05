# WS-LANG-01 Product UI Evidence — PKT-LSP-B (M8)

**Branch:** m8/lsp-read-ui
**Commit range:** 400396c..d731f63
**Date:** 2026-07-05 (fix round 2026-07-05)
**Session:** https://claude.ai/code/session_01HMw3X3iusfbbZhaWDm9Q4B

---

## Summary

PKT-LSP-B extended the WS-LANG-01 substrate foundation with a desktop product UI path.
Work is split into three test tiers captured below:

- **T6** — Completion popup desktop state machine (8 tests, all pass).
- **T7** — Hover tooltip + go-to-definition desktop state machine (7 tests, all pass).
- **T8** — Product composition smoke, `#[ignore]` gated; exercises `AppComposition`
  startup → diagnostics → completion projection → stale discard via `is_stale_response`.

The pre-existing `cargo run -p xtask -- rust-analyzer-smoke` xtask command runs the full
`legion-app::rust_analyzer_workflow` suite with `--ignored`, which now includes T8.

**Status:** Substrate-validated for the fixture path.  Product-ready claim is blocked on
real-server 3-OS smoke (deferred, same constraint as WS-LANG-01).  Write-side actions
(rename, code-action, format UI) are explicitly out of scope for this branch.

---

## Verification Table

### T6 — LSP completion popup desktop state machine

Command: `cargo test -p legion-desktop --test completion_popup`
CWD: `C:\Users\dasbl\RustroverProjects\legion-ide-lsp-b`
Start: 2026-07-04  End: 2026-07-04  Exit: 0

Trimmed output:

```
running 8 tests
test completion_dismiss_with_no_completions_is_noop ... ok
test completion_dismiss_closes_open_popup ... ok
test completion_next_with_no_completions_is_noop ... ok
test completion_next_wraps_around ... ok
test completion_prev_wraps_to_last ... ok
test completion_accept_with_no_completions_is_noop ... ok
test completion_accept_inserts_label_through_editor ... ok
test completion_popup_dismissed_on_tab_switch ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Contracts verified:
- `CompletionDismiss` with no data → Noop (no panic).
- `CompletionDismiss` closes open popup and calls `refresh_projection`.
- `CompletionNext` wraps from last → 0; `CompletionPrev` wraps from 0 → last.
- `CompletionAccept` inserts the selected label through editor authority → `Edited` outcome.
- `CompletionAccept` with no completions → Noop (guard condition).
- Tab switch dismisses stale popup (`completion_popup_dismissed_on_tab_switch`).
- Pre-sync of `last_completion_count` prevents re-open after dismiss.

---

### T7 — Hover tooltip + go-to-definition desktop state machine

Command: `cargo test -p legion-desktop --test hover_definition`
CWD: `C:\Users\dasbl\RustroverProjects\legion-ide-lsp-b`
Start: 2026-07-04  End: 2026-07-04  Exit: 0

Trimmed output:

```
running 7 tests
test hover_dismiss_with_no_hover_is_noop ... ok
test hover_dismiss_closes_open_tooltip ... ok
test hover_tooltip_shows_when_hover_data_arrives ... ok
test hover_tooltip_dismissed_on_tab_switch ... ok
test navigate_to_definition_with_no_definitions_is_noop ... ok
test go_to_definition_action_fires_language_tooling_request ... ok
test request_hover_action_fires_language_tooling_request ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Contracts verified:
- `HoverDismiss` with no data → Noop (no panic).
- `HoverDismiss` closes tooltip and pre-syncs `last_hover_id` so `refresh_projection`
  does not immediately re-open the same hover on the next frame.
- `set_hover_tooltip_visible_for_test(true)` syncs `last_hover_id` from current snapshot
  so tests leave consistent state.
- Tab switch clears `hover_tooltip_visible` but keeps `last_hover_id` so old tab's hover
  does not re-appear on the new tab.
- `hover_tooltip_shows_when_hover_data_arrives`: tooltip auto-shows when new hover id
  arrives and tooltip was not previously visible.
- `GoToDefinition` dispatches `LanguageToolingUpdated` through the bridge even without
  a live server.
- `NavigateToDefinition` with no definitions → Noop (guard condition).

---

### T8 — Product composition smoke (`#[ignore]`, requires rust-analyzer on PATH)

Command: `cargo run -p xtask -- rust-analyzer-smoke`
(internally: `cargo test -p legion-app --test rust_analyzer_workflow -- --ignored`)
CWD: `C:\Users\dasbl\RustroverProjects\legion-ide-lsp-b`
Gate: `#[ignore = "requires rust-analyzer on PATH; run with --ignored"]`
Test name: `rust_analyzer_product_composition_smoke`

Not run as part of this evidence capture (requires real rust-analyzer binary).
The test was added to `crates/legion-app/tests/rust_analyzer_workflow.rs` at commit
`d678395` and exercises:

1. Discovery — skip if `rust-analyzer` not on PATH.
2. `AppComposition::new()` + `open_workspace(WorkspaceTrustState::Trusted)`.
3. Drain LSP pump until `LspResultStatus::Fresh` or server-unavailable timeout.
4. `open_file()` to obtain an active `buffer_id`.
5. D2: `lsp_server_health_record()` is `Some` after a live startup.
6. D3: `language_tooling_projection.problems` is accessible.
7. `dispatch_ui_intent(CommandDispatchIntent::RequestCompletion { buffer_id, position })`
   + drain until completions arrive or timeout.
8. Stale discard: `is_stale_response(SnapshotId(1), SnapshotId(2)) == true`
   and `is_stale_response(SnapshotId(2), SnapshotId(2)) == false`.

For real-server evidence, run `cargo run -p xtask -- rust-analyzer-smoke` with
`rust-analyzer` available on PATH.  The existing WS-LANG-01 evidence
(`plans/evidence/production/WS-LANG-01/WS-LANG-01-evidence.md`) records a successful
real single-OS (Windows) smoke run covering the prior `rust_analyzer_full_workflow` test.

---

## Full legion-desktop test suite

Command: `cargo test -p legion-desktop`
CWD: `C:\Users\dasbl\RustroverProjects\legion-ide-lsp-b`
Start: 2026-07-04  End: 2026-07-04  Exit: 0
Result: All test suites pass; 0 failures.

(Full per-suite pass counts available in session output; targeted test counts above are
the load-bearing evidence for PKT-LSP-B.)

---

## Fix Round — 2026-07-05 (commit d731f63)

Fix-round addressing all findings from `lsp-b-review-report.md`.

### C1 — `unsafe set_var` removed

Both tests that previously called `unsafe { std::env::set_var }` now pass the mock
server path via `start_for_workspace_with_server_path(dir, true, Some(mock_path))`.
No environment mutation in the test process.

### I1 — Debounce state moved to `AppComposition`

Completion and hover debounce fields (`lsp_ui_completion_debounce`,
`lsp_ui_hover_debounce`, `lsp_ui_last_completion_count`, `lsp_ui_last_hover_id`)
moved from `DesktopRuntime` to `AppComposition`. Desktop now calls
`app.tick_lsp_debounces(Instant::now())` each frame and dispatches returned
`LspDebounceEvent` values. Methods added to `AppComposition`:
`arm_lsp_completion_debounce`, `disarm_lsp_completion_debounce`,
`arm_lsp_hover_debounce`, `disarm_lsp_hover_debounce`, `tick_lsp_debounces`,
`pre_sync_lsp_completion_count`, `pre_sync_lsp_hover_id`, `last_lsp_completion_count`,
`last_lsp_hover_id`.

### I2/T7 — Capability gating added

`lsp_server_supports_capability(capability: &str) -> bool` added to `AppComposition`
(fail-closed: empty capability list → `false`).  `issue_lsp_hover_request`,
`issue_lsp_definition_request`, and `issue_lsp_completion_request` now gate on the
respective capability before issuing.  Capabilities are parsed from the `initialize`
response JSON in `session.rs::initialize()`.

Verification: `cargo test -p legion-app --test app_lsp_composition`

```
running 17 tests
... (all ok)
test t7_capability_gated_requests_skip_when_unsupported ... ok
test t7_capability_gated_partial_support ... ok
test result: ok. 17 passed; 0 failed
```

### I3 — T3 edit→diagnostics cycle test

`t3_diagnostics_projection_add_then_clear_cycle` added to `app_lsp_composition.rs`.
Injects diagnostics, asserts non-empty projection, clears, asserts empty.

### I4 — T5 snapshot health-flow tests

- `t5_refused_health_in_snapshot`: asserts `Unavailable` record appears in
  `shell_projection_snapshot.language_tooling_projection.lsp_health_records` after
  a refused session.
- `t5_injected_live_health_in_snapshot`: asserts live health record (via
  `set_lsp_health_for_test`) appears in snapshot.

### T4 — Problems panel keyboard navigation

`ProblemNext`, `ProblemPrev`, `ProblemActivate` added to `DesktopAction` and wired in
`bridge.rs` and `workflow.rs`. `problems_selected_index: usize` tracks focused row in
`DesktopRuntime`; forwarded to `DesktopProjectionViewState` for rendering with `› ` prefix.

Verification: `cargo test -p legion-desktop --test keyboard_nav`

```
running 4 tests
test t4_problem_activate_with_no_problems_is_noop ... ok
test product_mode_switch_accepts_keyboard_activation ... ok
test t4_problem_next_increments_selection ... ok
test t4_problem_prev_decrements_selection ... ok
test result: ok. 4 passed; 0 failed
```

### M2 — Double-drain fixed

`assert!(!app.drain_lsp_session())` replaces the double-call assertion.

### M3 — `accept_completion` honors `insertText`

`insert_text: Option<String>` added to `LanguageCompletionProjection` (protocol DTO).
`completion_projection_for_item` in `legion-lsp` populates it from the LSP `insertText`
field when present and different from the label. `accept_completion` uses
`insert_text.as_deref().unwrap_or(&label)` for the inserted text.

### M4 — `lsp_health_rows` formatted-output test

`m4_lsp_health_rows_formatted_output` in `language_health_view.rs` injects a health
record via `AppComposition::set_lsp_health_for_test`, takes a snapshot, converts via
`DesktopProjectionViewModel::from_snapshot`, and asserts the row string contains
`lsp server=`, `provenance=`, `version=`, `status=`, `restarts=`, the injected version,
and "ready".

Verification: `cargo test -p legion-desktop --test language_health_view`

```
running 7 tests
... (all ok)
test m4_lsp_health_rows_formatted_output ... ok
test result: ok. 7 passed; 0 failed
```

### M5 — Delete/backspace re-arms completion debounce

`DesktopAction::DeleteRange { range }` added to `completion_debounce_info` match arm,
returning `Some(range.start)` so backspace/delete trigger debounce re-arm.

### Regression suite

- `cargo test -p legion-app --test app_lsp_composition` — 17/17 pass
- `cargo test -p legion-app --test rust_analyzer_session_handshake` — 2/2 pass
- `cargo test -p legion-desktop --test keyboard_nav` — 4/4 pass
- `cargo test -p legion-desktop --test language_health_view` — 7/7 pass
- `cargo test -p legion-desktop --test completion_popup` — 8/8 pass
- `cargo test -p legion-desktop --test hover_definition` — 7/7 pass
- `cargo test -p legion-protocol --test dto_contracts` — 111/111 pass
- `cargo test -p legion-lsp` — 8 pass, 1 ignored (live rust-analyzer smoke)
