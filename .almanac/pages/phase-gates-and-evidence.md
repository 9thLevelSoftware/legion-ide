---
title: Phase Gates And Evidence
summary: "Runtime activation in Legion IDE is controlled by dependency policy, `xtask` checks, ADR scope, and CLI evidence packages rather than by crate presence alone."
topics: [phase-gates, decisions, productization, architecture]
sources:
  - id: dependency-policy
    type: file
    path: plans/dependency-policy.md
    note: Defines internal dependency boundaries and phase-activation text.
  - id: xtask-main
    type: file
    path: xtask/src/main.rs
    note: Encodes evidence-path constants and acceptance checks.
  - id: cli-main
    type: file
    path: crates/legion-cli/src/main.rs
    note: Encodes evidence-check command markers and required artifacts for accepted phases.
  - id: gui-baseline
    type: file
    path: plans/evidence/gui-productization/gui-productization-baseline.md
    note: Explains how GUI productization layers on top of accepted substrate evidence.
status: active
verified: 2026-06-08
---
This repository does not treat a compiled crate as an accepted feature. Runtime activation is governed by `[[plans/dependency-policy.md]]`, `[[xtask/src/main.rs]]`, accepted ADRs, and evidence documents that must contain exact markers, artifacts, and checklists before a phase is considered accepted [@dependency-policy] [@xtask-main].

## What `xtask` enforces

`cargo run -p xtask -- check-deps` does more than check edges. It parses the dependency policy, hardcodes required paths and forbidden edges, and validates evidence packages for specific phases such as Phase 3, Phase 4, Phase 5, GUI Phase 5-8, legacy Phase 7/8, and Phase 13 [@xtask-main]. The gate is intentionally coupled to the policy text, so policy changes and `xtask` changes must move together.

## What the CLI enforces

`[[crates/legion-cli/src/main.rs]]` carries a second acceptance layer. It defines required evidence artifacts and required command markers for accepted phases such as Phase 8 and GUI Phase 6-8, including exact test names, smoke scripts, and evidence-check commands [@cli-main]. This makes the evidence package executable policy, not passive documentation.

## Why old evidence stays load-bearing

`[[plans/evidence/gui-productization/gui-productization-baseline.md]]` explains that GUI productization starts after accepted substrate evidence and must not reuse stale language that marks already accepted substrate phases as future-gated [@gui-baseline]. The dependency policy reflects the same idea by preserving historical accepted slices such as legacy plugin Phase 5 and remote-development Phase 7 while adding separate GUI acceptance tracks [@dependency-policy].

## Practical rule

Before enabling a new dependency, widening a crate edge, or treating a surface as product-ready:

- read the relevant ADR
- read the dependency-policy entry
- read the corresponding evidence package
- check whether `xtask` and `legion-cli evidence check` already enforce that claim

If one of those layers still says "deferred", the code is a scaffold or bounded harness, not an automatically approved runtime surface.
