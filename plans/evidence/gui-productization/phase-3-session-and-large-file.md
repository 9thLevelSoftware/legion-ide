# Phase 3 Session Restore And Large-File Guardrails Evidence

## Scope

Plan 03-05 adds desktop metadata-only session persistence/restore and verifies that large-file GUI rendering/search remain bounded.

## Commands

| Command | Result |
| --- | --- |
| `rg -q "pub mod session" crates/legion-desktop/src/lib.rs` | Passed |
| `rg -q "WorkspaceSessionRecord" crates/legion-desktop/src/session.rs` | Passed |
| `rg -q "schema_version" crates/legion-desktop/src/session.rs` | Passed |
| `rg -q "session-state" crates/legion-desktop/src/workflow.rs` | Passed |
| `rg -q "large_file_degraded_status" crates/legion-desktop/src/smoke.rs` | Passed |
| `cargo run -p xtask -- check-deps` | Passed; dependency policy accepted `legion-desktop -> serde_json` |
| `cargo fmt --all --check` | Passed |
| `cargo test -p legion-desktop session_restore -- --nocapture` | Passed; 4 tests passed |
| `cargo test -p legion-desktop large_file_guardrails -- --nocapture` | Passed; 2 tests passed |
| `cargo test -p legion-editor --test performance_suite -- --list` | Passed; listed 10 tests |
| `cargo test -p legion-desktop platform_smoke -- --nocapture` | Passed; 6 tests passed |
| `cargo test -p legion-desktop intent_bridge -- --nocapture` | Passed; 10 tests passed |
| `cargo check -p legion-desktop --all-targets` | Passed |
| `cargo clippy -p legion-desktop --all-targets -- -D warnings` | Passed |

## Session Restore Evidence

- `DesktopSessionStore` persists `WorkspaceSessionRecord` JSON only.
- Session JSON validation rejects `schema_version == 0`, empty `session_id`, and raw-source markers including `small_buffer_preview`, `source_body`, and dirty-body fixtures.
- `--session-state <path>` loads session metadata before an explicit `--file` is opened.
- Restore applies tabs through `AppComposition::restore_workspace_session_record`; desktop does not create editor buffers directly.
- Restored adapter state includes explorer expansion and panel metadata.
- Missing session files are no-op; corrupt JSON returns a typed session error; missing tab files are skipped and surfaced as warnings.
- `serde_json` was added from the existing workspace dependency set; `xtask check-deps` passed, and `Cargo.lock` changed only to record the new `legion-desktop` package dependency edge.

## Large-File Guardrail Evidence

- Desktop rendering tests open a file above the degraded threshold and assert:
  - active projection is degraded,
  - `small_buffer_preview` is absent,
  - viewport mode is `DegradedLargeFile`,
  - rendered rows come from bounded viewport slices.
- Active-file search on degraded buffers is limited to visible viewport content and does not find a marker placed outside the projected viewport.
- Smoke evidence fields now include `large_file_degraded_status`, `bounded_search_status`, and `full_text_projection_status`.

## Known Limitation

- The ignored 100MB performance workload remains a known degraded/streaming-mode gap. This plan lists the performance suite and verifies bounded desktop behavior; it does not claim the ignored 100MB workload passes.
