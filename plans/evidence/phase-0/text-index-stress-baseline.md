# Phase 0 Text and Index Stress Baseline

Status: Accepted with reservations

Accepted at: 2026-05-14T02:07:05Z

## Evidence artifacts

- `cargo-test-workspace-all-targets.txt` records successful non-ignored editor performance tests.
- `editor-performance-suite.txt` records archived ignored benchmark output and explains the current large-file and retained-history reservations.
- `cargo-check-workspace-all-targets.txt` records successful workspace target checking.
- `cargo-clippy-workspace-all-targets.txt` records successful warning-clean lint validation.

## Non-ignored performance baseline

| Test | Coverage | Acceptance result |
|---|---|---|
| `ci_typical_edit_latency_on_budget_sized_file` | Budget-sized file edit latency with p95 assertion below 250ms | Passed |
| `ci_snapshot_retention_budget_is_enforced` | Snapshot retention cap enforcement under edit burst | Passed |
| `ci_undo_redo_burst_small_deterministic_sample` | Undo/redo latency under deterministic retained-history sample | Passed |
| `invalid_batch_rolls_back_live_text_version_dirty_and_side_effects` | Batch atomicity and rollback on failed edit | Passed |
| `protocol_descriptor_preserves_utf16_ranges_for_surrogate_pairs` | UTF-16 range preservation in transaction descriptors | Passed |
| `failed_oversize_batch_preserves_undo_stack_and_transaction_log` | Oversize batch failure atomicity | Passed |

## Archived ignored benchmark baseline

The ignored benchmark suite output is archived in `editor-performance-suite.txt`.

Current reservations from that archive:

- The 100MB ignored workload currently triggers `FullCacheBudgetExceeded`, which is correct for the current full-cache budget and documents the absence of a degraded or streaming large-file path.
- The 2,000-edit ignored undo/redo workload exceeds the default bounded snapshot-retention assumption and must be rerun with an explicit large retention policy or rewritten to measure retained-history behavior intentionally.
- `snapshot_retention_and_release` passed in the ignored suite and remains a useful retained-snapshot smoke proof.

## Index baseline

The current `legion-index` crate remains a minimal placeholder in this milestone. The accepted baseline is therefore:

- No semantic indexing, embeddings, or vector retrieval are active in Phase 0.
- No index mutation path participates in editor keystroke latency.
- Future index responsiveness must be measured when the index actor and incremental update path are implemented.

## Conclusion

Track C is accepted with reservations. Non-ignored editor stress tests passed in the global workspace test run, and ignored benchmark output is archived to preserve the known 100MB and retained-history follow-up requirements.
