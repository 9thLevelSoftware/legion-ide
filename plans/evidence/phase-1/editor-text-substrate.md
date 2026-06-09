# Phase 1 Editor Text Substrate Evidence

Date: 2026-05-15

## Scope

This evidence records Phase 1 Workstream 5 coverage for streaming editor workloads. The measurements come from the focused local run of [`performance_suite.rs`](../../../crates/legion-editor/tests/performance_suite.rs) after replacing the old ignored 100MB full-cache workload with degraded-mode tests and fake-consumer simulations.

The Phase 1 guard from [`ADR-0015-streaming-text-viewport.md`](../../adrs/ADR-0015-streaming-text-viewport.md) is preserved: large buffers open in degraded mode, UI/app projections are viewport-first, snapshot leases are descriptor-only, and transaction streams remain metadata-only and bounded.

## Degraded-mode threshold and bounded viewport workload

Non-ignored CI workload: [`ci_large_file_degraded_open_and_viewport_are_bounded()`](../../../crates/legion-editor/tests/performance_suite.rs).

| Metric | Value |
| --- | ---: |
| Full-cache threshold | 5,242,880 bytes |
| Deterministic CI large file size | 5,373,952 bytes |
| Open time | 2.2404908s |
| Viewport projection time | 501.7us |
| Viewport payload bytes | 768 bytes |
| Snapshot chunk descriptors | 83 chunks |
| Overlay skip reasons | 3 reasons |

Assertions covered by the CI workload:

- Large content just above the full-cache threshold opens as degraded rather than constructing a full cache.
- Compatibility full-text access returns an error for the degraded buffer.
- Viewport projection mode is degraded large-file mode and carries only bounded line slices.
- Decoration, fold, and semantic-token overlays remain empty/deferred for the degraded viewport.
- Snapshot chunk descriptors are present and bounded by descriptors rather than full source transfer.

Skipped/deferred overlay reasons recorded by the large-file status:

- decorations skipped for degraded large files
- folds skipped for degraded large files
- semantic tokens skipped for degraded large files

## Small-buffer edit latency

Non-ignored CI workload: [`ci_typical_edit_latency_on_budget_sized_file()`](../../../crates/legion-editor/tests/performance_suite.rs).

| Metric | Value |
| --- | ---: |
| Buffer size | 256 KiB |
| Edit samples | 16 |
| p50 edit latency | 22.941ms |
| p95 edit latency | 23.3795ms |

The small-buffer path still validates normal editing remains below the 1s p95 latency ceiling while now reporting both p50 and p95.

## Fake background consumers

Non-ignored CI workload: [`ci_mixed_fake_consumers_do_not_block_user_edits()`](../../../crates/legion-editor/tests/performance_suite.rs).

The workload simulates LSP, index, AI retrieval, and collaboration replay using only protocol consumer kinds and editor APIs. It does not activate placeholder crates.

| Metric | Value |
| --- | ---: |
| Editor transaction event queue capacity | 8 descriptors |
| User edit cycles | 3 |
| User edits applied | 36 |
| Drained transaction descriptors | 24 |
| Engine dropped-before-drain descriptors | 12 |
| p50 user-edit latency | 109.8119ms |
| p95 user-edit latency | 130.7183ms |

Per fake-consumer bounded-queue counters:

| Consumer kind | Accepted | Dropped | Stale | Skipped versions | Queued after drain |
| --- | ---: | ---: | ---: | ---: | ---: |
| LSP | 7 | 17 | 0 | 8 | 4 |
| Index | 7 | 17 | 0 | 8 | 4 |
| AI retrieval | 7 | 17 | 0 | 8 | 4 |
| Collaboration replay | 7 | 17 | 0 | 8 | 4 |

Assertions covered by the workload:

- Every fake consumer acquires a descriptor-only snapshot lease through the editor API.
- The editor queue remains bounded and reports dropped-before-drain descriptors.
- Each fake consumer uses a bounded local queue and records drops instead of synchronously blocking edits.
- No stale event descriptors were observed by consumers.
- Skipped transaction versions are recorded when the bounded editor queue drops intermediate metadata-only events.
- User-edit p95 latency remains below the 1s guard while fake consumers lag and drop work.

## 100MB measurement-only workload

Ignored workload: [`large_file_100mb_degraded_mode_measurement()`](../../../crates/legion-editor/tests/performance_suite.rs).

This test remains explicitly ignored and measurement-only. It no longer expects green full-cache construction for a 100MB file; when run manually, it measures degraded-mode open, viewport, edit latency, viewport payload bytes, chunk count, threshold bytes, and overlay skip reasons.

## App/UI projection guard

App integration coverage: [`workspace_vfs_integration_large_file_projection_omits_full_source_text()`](../../../crates/legion-app/tests/workspace_vfs_integration.rs).

The app/UI projection test opens a deterministic file above the 5 MiB full-cache threshold and asserts:

- [`ActiveBufferProjection`](../../../crates/legion-ui/src/ui.rs) is marked degraded.
- `small_buffer_preview` / full-source text is absent.
- A viewport projection is present and uses degraded large-file mode.
- Viewport payload is bounded and overlays are empty/deferred.

## Save/conflict regression

The save/conflict regression remains focused on proposal-mediated saves through [`SaveWorkflowService::save_active_buffer()`](../../../crates/legion-app/src/lib.rs) and [`WorkspaceActor::save_file_with_proposal()`](../../../crates/legion-project/src/lib.rs). Workstream 5 did not change save ownership or conflict behavior.

Required focused validation command is recorded in the validation section below.

## Validation

Commands run locally:

- `cargo fmt --all` — pass.
- `cargo test -p legion-editor --test performance_suite -- --nocapture` — pass: 5 passed, 0 failed, 3 ignored, 0 measured, 0 filtered out; measured output listed above.
- `cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict` — pass: 1 passed, 0 failed, 0 ignored, 11 filtered out.
- `cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_large_file_projection_omits_full_source_text` — pass: 1 passed, 0 failed, 0 ignored, 11 filtered out.
