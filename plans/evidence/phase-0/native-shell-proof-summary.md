# Phase 0 Native Shell Proof Summary

Status: Accepted with reservations

Accepted at: 2026-05-14T02:07:05Z

Decision source: `plans/spikes/SPIKE-001A-result.md`

## Evidence basis

This summary is based on the definitive Phase 0 command evidence captured after the Phase 7 and Phase 8 migrations:

- `check-deps.txt`: `cargo run -p xtask -- check-deps` passed.
- `fmt-check.txt`: `cargo fmt --all --check` passed.
- `cargo-check-workspace-all-targets.txt`: `cargo check --workspace --all-targets` passed.
- `cargo-test-workspace-all-targets.txt`: `cargo test --workspace --all-targets` passed.
- `cargo-clippy-workspace-all-targets.txt`: `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `editor-performance-suite.txt`: archived ignored benchmark output and reservations for the 100MB and 2,000-edit ignored workloads.

## Native shell and editor-path measurements

| Metric | Evidence | Result | Reservation |
|---|---|---|---|
| Input-to-edit transaction latency | Non-ignored `ci_typical_edit_latency_on_budget_sized_file` in `crates/devil-editor/tests/performance_suite.rs` | Accepted: p95 assertion remained below 250ms in the full workspace test run | The current harness captures editor transaction latency rather than a compositor-backed input-to-paint span |
| p50 latency | Current accepted harness does not emit a p50 value in captured CI output | Accepted as a reporting gap, not a failing gate | Add p50 emission when the renderer-backed measurement harness lands |
| p95 latency | Current accepted harness asserts p95 below 250ms | Accepted | Use renderer-backed p95 once native painting is integrated |
| Frame variance | Full workspace test run completed without UI loop or projection instability | Accepted with reservations | No GPU/compositor frame-time harness exists in this phase |
| CPU utilization | Full command suite completed cleanly under local Windows execution | Accepted with reservations | No dedicated CPU sampler was attached to the headless proof |
| GPU utilization | The current shell proof exercises projection-only state and no GPU renderer | Accepted with reservations | GPU utilization is reserved for the native renderer integration proof |
| Memory usage | Non-ignored snapshot retention tests passed; ignored 100MB output documents `FullCacheBudgetExceeded` as intentional current large-file boundary behavior | Accepted | Streaming/degraded large-file mode remains a later performance harness requirement |
| IME | UI command dispatch is projection-only and does not own editor state | Accepted with reservations | Native IME event-loop validation remains a renderer integration follow-up |
| Clipboard | UI command parsing and dispatch intent paths compile and test cleanly | Accepted with reservations | Native clipboard API proof remains a renderer integration follow-up |
| Focus | Shell state is projection-only and receives snapshots rather than owning editor sessions | Accepted with reservations | Native window focus measurement remains a renderer integration follow-up |
| Command palette | Command intent routing is validated by application tests that dispatch UI intents through `EditorEngine` and `WorkspaceActor` | Accepted | Future command-palette UI can consume the same intent path |
| Accessibility | Projection snapshots are deterministic and renderer-independent | Accepted with reservations | Native accessibility tree integration remains a renderer integration follow-up |

## Summary

The Phase 0 native shell proof is accepted with reservations because all definitive workspace gates passed and the UI/editor migration now enforces projection-only UI state plus engine/workspace command routing. Renderer-specific p50, frame variance, GPU, IME, clipboard, focus, and accessibility measurements are documented as owned follow-ups rather than blockers to this acceptance point.
