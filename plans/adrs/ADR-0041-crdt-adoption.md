# ADR-0041: CRDT Adoption for the Anchor Layer

## Status

Accepted — post-GA collaboration decision for WS16.T1 on 2026-06-13.

## Context

WS16.T1 asked for a concrete decision between Loro, yrs, and a homegrown anchor-layer implementation over the existing operation-log runtime. The project already has a deterministic collaboration runtime (`legion-collaboration`) and protocol-backed position/operation contracts from ADR-0040, but it does not yet vendor any external CRDT crate. That means the decision is not about whether the anchor layer exists; it is about whether the anchor layer should become a CRDT-backed subsystem or remain a focused, local, protocol-mediated runtime.

The current baseline is the existing operation-log runtime exercised through the Legion-Bench v0 suite and the collaboration contract tests. Those surfaces already demonstrate the invariants the anchor layer needs to preserve: deterministic replay, fail-closed conflict handling, metadata-first records, and preservation of the proposal-mediated save boundary.

## Decision

Adopt the homegrown anchor-layer approach over the existing operation-log runtime. Do not introduce Loro or yrs as the anchor-layer substrate at this stage.

In practice, that means:

- keep the anchor layer protocol-mediated and deterministic inside the existing Legion runtime boundaries;
- keep collaboration state and replay logic in `legion-collaboration` and its protocol DTOs rather than migrating the anchor layer to an external CRDT engine;
- defer any future external CRDT evaluation until there is a measured workload that demonstrates the homegrown runtime cannot satisfy the collaboration target.

## Why this decision

1. The existing runtime already satisfies the evidence target without a CRDT dependency.
2. The benchmark harness shows the current operation-log baseline is stable and repeatable.
3. The repository currently has no `loro`, `yrs`, `diamond-types`, or `automerge` dependency surface to justify a swap based on existing code evidence.
4. A homegrown anchor layer keeps the protocol boundary and failure semantics under direct control, which is important for proposal-mediated saves and metadata-first observability.

## Benchmark evidence

The Legion-Bench v0 baseline was run in recorded-offline mode against the current `legion-desktop` package.

Command:

```bash
cargo test -p xtask --test legion_bench -- --nocapture
cargo run -p xtask -- legion-bench --out target/legion-bench --mode recorded
cargo run -p xtask -- verify-legion-bench --out target/legion-bench
```

Results:

- 5/5 xtask bench tests passed.
- Legion-Bench report summary: 20 total / 20 passed / 0 failed / 0 regressed.
- Average score: 61.
- Report path: `target/legion-bench/legion_bench_report.toml`.
- Suite fingerprint: `bench-suite-v1:bd2aa3a7d84d9485`.
- Git SHA captured in report: `c81eabeba532abb831bf55b6e5419f50f7a727e2`.

Evidence note: `plans/evidence/production/M6/WS16-T1-crdt-adoption.md`

## Rejected alternatives

- Loro: not adopted because there is no demonstrated need to trade the current deterministic runtime for an external CRDT dependency.
- yrs: not adopted for the same reason; no evidence currently shows it is required to satisfy the collaboration target.
- Hybrid CRDT swap now, homegrown later: rejected because it would add dependency and migration cost before any measured gap exists.

## Consequences

- The anchor layer remains a Legion-owned runtime concern instead of a vendored CRDT dependency.
- Future collaboration work can still revisit Loro or yrs if a workload-backed benchmark shows the homegrown path is insufficient.
- WS16.T2 and WS16.T3 continue to build on the existing operation-log substrate rather than on a new external CRDT core.
