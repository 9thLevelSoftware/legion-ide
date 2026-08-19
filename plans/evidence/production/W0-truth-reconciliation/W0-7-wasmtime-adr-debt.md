# W0.7 — Wasmtime supply-chain ADR debt (P7 entry blocker)

Date: 2026-08-05
Branch: `wave0/backlog-truth-reconciliation`
Backlog card: `P7.F1.T1`

## Finding

`plans/adrs/ADR-0019-wasm-plugin-runtime.md` line 26 states:

> A future Wasmtime/WASI engine may be added only after supply-chain review and
> evidence proves equivalent no-ambient-authority behavior.

Wasmtime is already a dependency. `crates/legion-plugin/Cargo.toml:16` declares
`wasmtime = { workspace = true }`, and `crates/legion-plugin/src/host.rs`
implements a `WasmPluginHost` on top of it.

No ADR authorizes that dependency, and `plans/dependency-policy.md` does not
mention wasmtime — its `legion-plugin` section (lines 182-189) still describes
the Phase 5 boundary in engine-agnostic terms.

Backlog card `P7.F1.T1` carries the stop condition:

> Stop if the runtime is added to the workspace before the ADR is merged.

**That stop condition is already violated.** The card is recorded `todo`, which
is accurate for the ADR itself, but the ordering it was written to protect has
already been lost.

## Why this is the P7 entry blocker

The plugin runtime is otherwise well ahead of its backlog status. The crate has
a real capability-checked host with fail-closed quota and trap handling, four
hostile `.wat` fixtures (`fixtures/hostile/{loop,oom,capability_probe,workspace_access}.wat`),
and tests in `tests/{hostile,quotas,tampered,wit_abi}.rs`. What it does not have
is authorization: no product binary reaches `WasmPluginHost` (`legion-app/src/lib.rs:89`
imports the metadata-only `PluginRuntimeHost` instead), and no ratified decision
says wasmtime may be there at all.

Wiring an unauthorized engine into the product composition root would convert a
paperwork gap into a shipped capability. So the ADR is sequenced first and alone
(W4.1), before the ABI decision, host wiring, or extensions panel.

## Required to clear

1. An ADR that supersedes ADR-0019's "future engine" language and ratifies
   wasmtime explicitly, including the supply-chain review the original ADR
   demanded and the no-ambient-authority argument (no WASI imports; single
   audited `env::host_log`; capability checks through `DenyByDefaultBroker`).
2. A `plans/dependency-policy.md` entry for wasmtime under `legion-plugin`.
3. An explicit record — this file — that the dependency preceded the decision,
   so the sequence is auditable rather than quietly reordered.

Items 1 and 2 are W4.1. Item 3 is this note.

## Status

**Cleared 2026-08-19.** All three items are done:

1. `plans/adrs/ADR-0050-wasmtime-runtime-ratification.md` supersedes ADR-0019's
   "future engine" clause, performs the supply-chain review, and ratifies
   wasmtime with four conditions on the grant.
2. `plans/dependency-policy.md` now carries a plugin-runtime engine entry in the
   `legion-plugin` section.
3. This file records that the dependency preceded the decision. The ordering
   finding is restated precisely in ADR-0050 §Context: ADR-0019 *did* merge five
   weeks before `236a492` added wasmtime, so the stop condition's literal words
   were met — but ADR-0019 withheld authorization, so the runtime landed against
   an ADR that said "not yet". The intent was violated; the letter was not.

The paperwork gap is additionally closed against recurrence:
`validate_plugin_runtime_dependency_gate` in `xtask` fails
`cargo run -p xtask -- check-deps` if wasmtime is in the workspace while the
ADR or the policy clause is absent, or if any crate other than `legion-plugin`
declares it. See `plans/evidence/production/P7-F1/wasmtime-adr-and-wit-abi.md`.
