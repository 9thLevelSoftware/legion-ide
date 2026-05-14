# Milestone 0: Spike 1A Feasibility Proofs

## Status

Accepted

Accepted at: 2026-05-14T02:07:05Z

## Purpose

Milestone 0 validates Spike 1A viability before broad feature implementation in other domains. It focuses on architecture direction, platform boundaries, and text/index performance under representative load.

## Scope

- Spike 1A native editor shell latency path.
- Required contract hardening for editor/project boundary.
- Freeze criteria enforcement before scaling.

## Proof tracks

### Track A — UI Native Path Validation

Status: Accepted with reservations

Evidence:
- `plans/SPIKE-001A-native-shell-proof.md`
- `plans/spikes/SPIKE-001A-result.md`
- `plans/evidence/phase-0/native-shell-proof-summary.md`

Result:
- Editor and application command paths route UI intents through projection snapshots, `EditorEngine`, and `WorkspaceActor`.
- SPIKE-001A decision is `PASS WITH RESERVATIONS` because compositor-backed p50/frame/GPU/IME/clipboard/accessibility measurements are owned follow-ups.

### Track B — Boundary and Dependency Direction Validation

Status: Accepted

Evidence:
- `plans/architecture-freeze-v0.1.md`
- `plans/evidence/phase-0/check-deps.txt`
- `plans/evidence/phase-0/cargo-test-workspace-all-targets.txt`

Result:
- Dependency direction validation passed.
- Protocol DTO and event-envelope tests passed.
- Editor/project boundaries remain mediated by protocol and workspace/editor ports.

### Track C — Text + Index Stress Baseline

Status: Accepted with reservations

Evidence:
- `plans/evidence/phase-0/text-index-stress-baseline.md`
- `plans/evidence/phase-0/cargo-test-workspace-all-targets.txt`
- `plans/evidence/phase-0/editor-performance-suite.txt`

Result:
- Non-ignored performance tests `ci_typical_edit_latency_on_budget_sized_file`, `ci_snapshot_retention_budget_is_enforced`, and `ci_undo_redo_burst_small_deterministic_sample` passed.
- Atomicity and UTF-16 transaction descriptor tests passed.
- Archived ignored benchmark output records the 100MB full-cache boundary and retained-history reservations without treating them as green benchmark data.
- Index responsiveness remains a baseline no-op because `devil-index` is not active in Phase 0.

### Track D — Platform Boundary Verification

Status: Accepted

Evidence:
- `plans/SPIKE-000-platform-boundary-proof.md`
- `plans/evidence/phase-0/platform-boundary-api-map.md`
- `plans/evidence/phase-0/cargo-check-workspace-all-targets.txt`
- `plans/evidence/phase-0/cargo-test-workspace-all-targets.txt`

Result:
- Every public `devil-platform` API is mapped to an OS concern.
- Editor, window, model, and request-routing ownership are explicitly excluded from platform ownership.

## Milestone exit criteria

- All tracks have linked evidence artifacts.
- Architecture freeze conditions are accepted in `plans/architecture-freeze-v0.1.md`.
- Reservations are documented with owner roles and follow-up criteria.
- No unresolved blocker is treated as cleared without evidence.

## Notes

- Minimal scaffold crates can now grow only through the approved phase sequence and global validation gates.
