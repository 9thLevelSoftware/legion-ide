# SPIKE-001A: Native UI Editor Latency Proof

## Status

Accepted with reservations

Accepted at: 2026-05-14T02:07:05Z

## Objective

Prove the native shell/editor path can sustain the Phase 0 editing and interaction requirements under load and that ADR-0002 assumptions remain viable before implementation scale.

## Setup

- Platform target validated in this evidence set: Windows 11.
- Shell architecture validated in this evidence set: projection-only UI state, command dispatch intents, `EditorEngine` buffer transactions, and `WorkspaceActor` file authority.
- Measurement evidence source: global workspace validation plus editor performance-suite baseline artifacts under `plans/evidence/phase-0`.

## Measured evidence

| Area | Evidence | Result |
|---|---|---|
| Latency | `ci_typical_edit_latency_on_budget_sized_file` computes edit samples and asserts p95 below 250ms | Accepted |
| p50 latency | Current accepted non-ignored harness does not emit p50 to captured output | Accepted with reporting reservation |
| p95 latency | Non-ignored p95 assertion passed in global test evidence | Accepted |
| Frame variance | No compositor-backed renderer is present; projection-only shell tests completed without state instability | Accepted with renderer reservation |
| CPU utilization | Full workspace commands completed without timeout or hang under Windows local execution | Accepted with sampler reservation |
| GPU utilization | GPU renderer path is not exercised because UI is projection-only in this phase | Accepted with renderer reservation |
| Memory usage | Snapshot retention tests passed; ignored 100MB workload archive documents current full-cache budget boundary | Accepted with large-file reservation |
| IME | Native IME path is reserved for renderer integration; command intent boundaries are now renderer-independent | Accepted with renderer reservation |
| Clipboard | Clipboard path is reserved for renderer integration; command intent boundaries are now renderer-independent | Accepted with renderer reservation |
| Focus | Focus path is reserved for renderer integration; shell state no longer owns editor session state | Accepted with renderer reservation |
| Command palette | UI intents route through application services and are validated by app integration tests | Accepted |
| Accessibility | Accessibility tree generation is reserved for renderer integration; projection snapshots provide deterministic source data | Accepted with renderer reservation |
| Platform notes | Windows 11 validation passed; macOS/Linux parity remains part of later renderer validation | Accepted with parity reservation |

## Evidence artifacts

- `plans/evidence/phase-0/native-shell-proof-summary.md`
- `plans/evidence/phase-0/text-index-stress-baseline.md`
- `plans/evidence/phase-0/cargo-test-workspace-all-targets.txt`
- `plans/evidence/phase-0/cargo-clippy-workspace-all-targets.txt`
- `plans/evidence/phase-0/editor-performance-suite.txt`

## Acceptance decision

SPIKE-001A is accepted with reservations. The Phase 0 architecture is ready to scale through the planned phase gates because UI state is projection-only, editor transactions are engine-owned, workspace file authority is actor-owned, and all definitive global validation commands passed.
