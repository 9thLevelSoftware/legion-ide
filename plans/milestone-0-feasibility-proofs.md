# Milestone 0: Spike 1A Feasibility Proofs

## Status

Draft, blocking implementation milestone.

## Purpose

Milestone 0 validates Spike 1A viability before any broad feature implementation in other domains. It focuses on architecture direction, platform boundaries, and text/index performance under representative load.

## Scope

- Spike 1A (Native editor shell latency path)
- Required contract hardening for editor/project boundary
- Freeze criteria enforcement before scaling

## Proof Tracks

### Track A — UI Native Path Validation (`SPIKE-001A`)

- Validate editor latency and interaction quality on target platforms.
- Validate IME/clipboard/scroll/text input feasibility.
- Confirm accessibility and input/event-loop integration feasibility.
- Produce proof report with metrics and pass/fail decision.

Acceptance:
- Pass conditions from `plans/SPIKE-001A-native-shell-proof.md` are met.

### Track B — Boundary and Dependency Direction Validation

- Validate dependency inversion for `devil-ai` vs `devil-ai-providers`.
- Validate no hard editor→project dependency via direct crate edge.
- Validate protocol contracts for `ProjectInfo*` and editor transaction flow in `devil-protocol`.

Acceptance:
- Checks in `plans/architecture-freeze-v0.1.md` gates 1 and 2 are satisfied.

### Track C — Text + Index Stress Baseline

- Validate large-file edit throughput and undo/redo under load.
- Validate snapshot memory growth and rollback behavior at scale.
- Validate index update responsiveness while typing on large files.

Acceptance:
- Metrics in `plans/architecture-charter-v0.1.md §16.8` are collected and reviewed.

### Track D — Platform Boundary Verification

- Validate that `devil-platform` is constrained to OS abstractions and no editor-level ownership.
- Validate service interfaces are documented and stable for platform duties.

Acceptance:
- `plans/SPIKE-000-platform-boundary-proof.md` is completed and reviewed.

## Milestone Exit Criteria

- All tracks above have evidence artifacts.
- Architecture freeze conditions from `plans/architecture-freeze-v0.1.md` are met.
- Any unresolved risks are explicitly logged with owners and mitigation before moving to Milestone 1.

## Notes

- Keep implementation of `devil-agent`, `devil-memory`, `devil-observability`, and `devil-cli` at minimum viable scaffolding until this milestone is passed.
