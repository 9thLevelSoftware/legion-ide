# M6 — WS16.T3 Cloud Lane Productization Evidence

## Status

Accepted.

## Acceptance target

- Hosted worker capacity with visible upload scope, budget, and cancellation.
- Existing HTTP transport + contract docs as the base.
- Cloud-executed Delegate task with full egress visibility.

## What was verified

- `docs/LEGION_PIVOT.md`
  - Defines Cloud Lane as opt-in hosted worker capacity with visible upload scope, cost/budget limits, and cancellation.
  - Places Cloud Lane alongside Delegate and Automate as a product primitive, not a hidden transport detail.
- `crates/legion-security/src/lib.rs`
  - `CloudLaneSecurityPolicy` requires trusted workspaces and keeps task submission, event streaming, cancellation, and artifact fetch disabled by default.
  - `cloud_lane_capability_decision()` enforces visible upload scope, validated task packets, hard cost caps, upload-byte caps, and HTTPS network-target checks before Cloud Lane submission is allowed.
- `crates/legion-remote/src/lib.rs`
  - `LegionCloudLaneClient` exposes the Cloud Lane control-plane surface: `submit_task`, `stream_task_events`, `cancel_task`, `fetch_task_proposal`, and `fetch_task_evidence`.
  - The client validates task IDs, response IDs, and evidence/proposal contracts before returning data to callers.
  - `HttpLegionCloudLaneTransport` is the production HTTP JSON transport implementation behind that client.
- `crates/legion-remote/tests/cloud_lane_http_transport.rs`
  - Proves the HTTP transport sends the expected submit/stream/cancel/proposal/evidence requests.
  - Verifies redacted auth-token handling, client-identity headers, visible upload scope, cost-cap rejection before network, forbidden-upload rejection before network, and HTTP error classification.
- `crates/legion-agent/src/lib.rs`
  - `DelegatedTaskSandboxOrchestrator` and `DelegatedTaskProposalGenerator` keep delegated-task execution sandboxed and proposal-oriented.
  - The approval-gated write path and proposal generation test exercise the Delegate substrate that Cloud Lane relies on for cloud-executed work.

## Verification commands

```bash
cargo test -p legion-remote --test cloud_lane_http_transport -- --nocapture
cargo test -p legion-security cloud_lane_submit_requires_visible_scope_budget_cap_and_https_target -- --nocapture
cargo test -p legion-agent test_sandbox_orchestration_and_containment_and_proposal_generation -- --nocapture
```

## Results

- `cargo test -p legion-remote --test cloud_lane_http_transport -- --nocapture`
  - 11 tests passed.
- `cargo test -p legion-security cloud_lane_submit_requires_visible_scope_budget_cap_and_https_target -- --nocapture`
  - 1 test passed.
- `cargo test -p legion-agent test_sandbox_orchestration_and_containment_and_proposal_generation -- --nocapture`
  - 1 test passed.

## Findings

- Cloud Lane is productized as a bounded, metadata-visible control plane: upload scope, budget, cancellation, proposal fetch, evidence fetch, and redacted auth are all enforced and covered.
- The delegated-task path remains app-owned, sandboxed, and permission-gated, which matches the Delegate side of the acceptance.
- The HTTP transport and contract-layer evidence show the Cloud Lane surface is not a mock-only slice anymore; it is wired through real request/response behavior with deterministic tests.
