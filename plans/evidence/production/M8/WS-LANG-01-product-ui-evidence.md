# WS-LANG-01 Product UI Evidence — PKT-LSP-B (M8)

**Branch:** m8/lsp-read-ui
**Commit range:** 400396c..d678395
**Date:** 2026-07-04
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
