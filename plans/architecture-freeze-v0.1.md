# Architecture Freeze: Legion IDE Spike 1A Prerequisites v0.1

## Status

Accepted

Accepted at: 2026-05-14T02:07:05Z

## Scope

This freeze records the contractual, architectural, and sequencing evidence required before implementation can scale beyond Spike 1A baseline work.

## Gate acceptance table

| Gate | Evidence Artifact | Command or Test | Result | Owner Role | Accepted At |
|---|---|---|---|---|---|
| Dependency direction validation | `plans/evidence/phase-0/check-deps.txt` | `cargo run -p xtask -- check-deps` | Passed | Architecture + dependency policy owner | 2026-05-14T02:07:05Z |
| Protocol contract stability | `plans/evidence/phase-0/cargo-test-workspace-all-targets.txt` | `cargo test --workspace --all-targets` protocol DTO tests | Passed | Protocol owner | 2026-05-14T02:07:05Z |
| Text-model stress validation | `plans/evidence/phase-0/text-index-stress-baseline.md` | Non-ignored editor performance and atomicity tests | Passed with recorded reservations for ignored heavy benchmarks | Editor + text owner | 2026-05-14T02:07:05Z |
| Platform boundary proofing | `plans/evidence/phase-0/platform-boundary-api-map.md` | `cargo check --workspace --all-targets` and `cargo test --workspace --all-targets` platform boundary slice | Passed | Platform + security owner | 2026-05-14T02:07:05Z |
| UI native shell dependency | `plans/evidence/phase-0/native-shell-proof-summary.md` and `plans/spikes/SPIKE-001A-result.md` | UI/app command-intent tests plus editor latency baseline tests | Passed with Spike 1A reservations | UI/runtime + architecture owner | 2026-05-14T02:07:05Z |
| Repository health gate | `plans/evidence/phase-0/fmt-check.txt`, `plans/evidence/phase-0/cargo-check-workspace-all-targets.txt`, `plans/evidence/phase-0/cargo-test-workspace-all-targets.txt`, `plans/evidence/phase-0/cargo-clippy-workspace-all-targets.txt` | `cargo fmt --all --check`; `cargo check --workspace --all-targets`; `cargo test --workspace --all-targets`; `cargo clippy --workspace --all-targets -- -D warnings` | Passed | QA + release owner | 2026-05-14T02:07:05Z |

## Accepted conditions

- Dependency inversion and dependency policy are enforced by the archived `xtask` run.
- Protocol DTO, text transaction, event envelope, storage, security, workspace, editor, UI, and app tests all pass in the workspace test evidence.
- Platform API ownership is accepted as OS-only and explicitly excludes editor state, window ownership, model authority, and request routing.
- SPIKE-001A is accepted with reservations documented in the result artifact and native shell proof summary.
- The repository health baseline is accepted with formatting, checking, testing, and warning-clean lint evidence.

## Freeze decision

The architecture freeze is accepted for Phase 0 evidence closure. Broad implementation may proceed only through the phase sequence and validation gates defined by the roadmap and implementation plan.
