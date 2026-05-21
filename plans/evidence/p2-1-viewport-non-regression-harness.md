# P2.1 viewport non-regression harness evidence

Date: 2026-05-21

Scope: P2.1 only. This note records non-regression coverage for ADR-0015 viewport-first rendering and text snapshot bounded access. It does not mark a broader phase accepted.

## Coverage added or strengthened

- [`workspace_vfs_integration_large_file_projection_omits_full_source_text()`](../../crates/devil-app/tests/workspace_vfs_integration.rs) now checks both active-buffer and shell projection snapshots for degraded large-file mode, absent `small_buffer_preview`, bounded viewport payload bytes, chunked/degraded status text, and empty expensive overlays.
- [`workspace_vfs_integration_small_buffer_projection_keeps_bounded_preview()`](../../crates/devil-app/tests/workspace_vfs_integration.rs) preserves the explicit bounded small-buffer exception: small buffers still expose preview text while large degraded buffers do not.
- [`shell_snapshot_large_file_projection_carries_only_viewport_slices()`](../../crates/devil-ui/src/ui.rs) locks the UI shell snapshot shape around degraded viewport line slices and no full-source preview.
- [`opening_larger_than_budget_uses_degraded_cache_free_mode()`](../../crates/devil-text/src/lib.rs) now validates chunk descriptor range continuity, bounded chunk byte lengths, ordinals, and content hashes for above-budget buffers and snapshots.
- [`large_snapshot_line_slices_and_chunks_are_bounded_by_default()`](../../crates/devil-text/src/lib.rs) proves above-budget snapshots reject full-text compatibility access while still exposing bounded chunk reads and bounded visible line slices.
- [`ci_large_file_degraded_open_and_viewport_are_bounded()`](../../crates/devil-editor/tests/performance_suite.rs) now also asserts each viewport line slice is smaller than the large source and the total viewport payload remains far below full-source size.

## Validation run

- `cargo run -p xtask -- check-deps`: passed.
- `cargo fmt --all --check`: initially reported formatting drift; `cargo fmt --all` was run, then `cargo fmt --all --check` passed.
- Targeted viewport/text/UI projection tests: passed:
  - `cargo test -p devil-ui`
  - `cargo test -p devil-text large_snapshot_line_slices_and_chunks_are_bounded_by_default`
  - `cargo test -p devil-text opening_larger_than_budget_uses_degraded_cache_free_mode`
  - `cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_large_file_projection_omits_full_source_text`
  - `cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_small_buffer_projection_keeps_bounded_preview`
  - `cargo test -p devil-editor --test performance_suite ci_large_file_degraded_open_and_viewport_are_bounded`
- `cargo check --workspace --all-targets`: passed.
- `cargo test --workspace --all-targets`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo deny check`: unavailable in this environment (`cargo` reported `no such command: deny`).

## P2.1-only confirmation

This slice added test harness and evidence coverage only. It did not implement P2.2 snapshot lease consumer contracts, P3 semantic-index remediation, LSP, AI, agent, plugin, terminal, remote, collaboration runtime behavior, or alternate write paths.
