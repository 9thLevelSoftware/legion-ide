---
title: Bounded Runtime Surfaces
summary: "Plugin, collaboration, remote, terminal, telemetry, and retention crates are real code paths, but each one is deliberately scoped, metadata-first, and productized through separate evidence gates."
topics: [advanced-surfaces, architecture, productization, decisions]
sources:
  - id: dependency-policy
    type: file
    path: plans/dependency-policy.md
    note: Defines the allowed edges and activation constraints for plugin, collaboration, remote, terminal, telemetry, and retention crates.
  - id: plugin-lib
    type: file
    path: crates/legion-plugin/src/lib.rs
    note: States the WASM runtime boundary and fail-closed host-authority rule.
  - id: collaboration-lib
    type: file
    path: crates/legion-collaboration/src/lib.rs
    note: States the deterministic metadata-first collaboration runtime boundary.
  - id: remote-lib
    type: file
    path: crates/legion-remote/src/lib.rs
    note: States the metadata-first remote runtime harness boundary.
  - id: remote-transport-lib
    type: file
    path: crates/legion-remote-transport/src/lib.rs
    note: States the production-gated transport boundary.
  - id: terminal-lib
    type: file
    path: crates/legion-terminal/src/lib.rs
    note: States the deterministic local terminal fixture boundary.
  - id: retention-lib
    type: file
    path: crates/legion-retention/src/lib.rs
    note: States the raw-source retention fixture and vault primitive boundary.
  - id: telemetry-lib
    type: file
    path: crates/legion-telemetry/src/lib.rs
    note: States the hosted telemetry spool/export fixture boundary.
  - id: terminal-tests
    type: file
    path: crates/legion-app/tests/terminal_workflow.rs
    note: Verifies default-denied and non-mutating terminal behavior.
status: active
verified: 2026-06-08
---
Several crates in this workspace are easy to over-read because they are no longer empty placeholders. They do implement behavior. They are still bounded surfaces whose scope is set by policy, tests, and evidence, not by their existence alone [@dependency-policy].

## Individual boundaries

`[[crates/legion-plugin/src/lib.rs]]` is a Phase 5 WASM runtime boundary that validates manifests, capability metadata, and quotas without granting ambient host authority [@plugin-lib]. `[[crates/legion-collaboration/src/lib.rs]]` is a deterministic metadata-first collaboration operation log runtime [@collaboration-lib]. `[[crates/legion-remote/src/lib.rs]]` is a metadata-first remote development harness, while `[[crates/legion-remote-transport/src/lib.rs]]` is the production-gated transport carrier layer [@remote-lib] [@remote-transport-lib].

`[[crates/legion-terminal/src/lib.rs]]` is still described as a deterministic local terminal fixture runtime, not an unrestricted terminal subsystem [@terminal-lib]. The app tests make that concrete: terminal launch is denied by default, untrusted workspaces are denied even with the fixture enabled, and terminal actions must not mutate editor text or disk state [@terminal-tests].

`[[crates/legion-retention/src/lib.rs]]` and `[[crates/legion-telemetry/src/lib.rs]]` expose raw-source retention and hosted telemetry primitives, but both are framed as Phase 8 fixtures or production-gated surfaces with explicit consent and policy shape, not as default-on storage channels [@retention-lib] [@telemetry-lib].

## Practical interpretation

If you need one of these surfaces, do not start from "the crate already exists". Start from:

- the dependency-policy entry
- the accepted ADR or evidence package for that phase
- the tests that show the current allowed behavior

This is the shared pattern behind terminal denial, metadata-only collaboration, remote proposal gating, plugin sandboxing, and retention/telemetry consent constraints.
