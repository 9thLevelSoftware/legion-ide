# Task Packet: cloud-transport-01 — Implement HTTP JSON Cloud Lane transport

## Project

- Name: legion-ide
- Repository: /Users/christopherwilloughby/devil-ide
- Coordinator: GPT-5.5
- Implementer: Kimi 2.6

## Phase

- ID: phase-3-cloud-transport
- Title: Production HTTP cloud lane transport
- Objective: Add a production-capable HTTP JSON transport for Cloud Lane while preserving policy gates, metadata-only boundaries, and proposal-only semantics.

## Origin

- Origin: audit_gap
- Source finding IDs: finding-cloud-lane-production-transport-missing

## Objective

Implement reqwest-blocking transport for Cloud Lane submit, stream, cancel, proposal, and evidence endpoints with deterministic local server tests.

## Dependencies

- None

## Allowed Files

- `crates/legion-remote/src/lib.rs`
- `crates/legion-remote/Cargo.toml`
- `crates/legion-remote/tests/cloud_lane_http_transport.rs`
- `plans/evidence/legion-e2e/2026-06-03_cloud_transport_contract.md`
- `plans/evidence/legion-e2e/2026-06-03_cloud_lane_http_transport_gates.txt`
- `Cargo.lock`

## Forbidden Files

- `.github/workflows/ci.yml`
- `crates/legion-desktop/**`
- `training/**`
- `evals/**`

## Required Context

- LegionCloudLaneClient already performs policy validation before calling the transport.
- Transport must not log raw bodies or secrets.

## Implementation Steps

- Add transport config with base URL, timeout, identity label, and redacted auth token support.
- Implement trait methods over deterministic JSON endpoints.
- Add TcpListener-based tests for success, headers, policy-before-network, and HTTP error classification.
- Document transport contract and policy boundaries.

## Targeted Tests

- `cargo test -p legion-remote --test cloud_lane_http_transport -- --nocapture`
- `cargo test -p legion-remote --all-targets`
- `cargo run -p xtask -- check-deps`

## Acceptance Criteria

- HTTP transport tests pass.
- Policy rejects disabled/cost/upload failures before network.
- Auth token is redacted from Debug output.
- Contract document references exact DTOs and endpoints.

## Definition of Done

- Targeted remote tests pass.
- Dependency policy gate passes.
- No app wiring or config enablement is added.

## Known Risks

- reqwest/rustls dependency changes may affect workspace policy.

## Stop Conditions

Stop and report if any of these occur:

- Need protocol DTO redesign.
- Dependency policy requires broad changes.
- Targeted tests fail after two fix attempts.
- Task exceeds 45 minutes.

## Timebox

45 minutes.

## Output Format Required

- Summary
- Files changed
- Tests run and exact results
- Acceptance checklist
- Blockers or deviations

## Hard Rules

- Implement only this task packet.
- Modify only allowed files.
- Do not create branches, commit, push, open PRs, merge PRs, or modify CI unless explicitly listed in allowed files.
- Do not broaden scope to adjacent tasks.
- Stop after two failed fix attempts.
