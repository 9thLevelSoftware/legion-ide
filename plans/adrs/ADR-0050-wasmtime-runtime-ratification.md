# ADR-0050: Wasmtime as the Legion Plugin Runtime

## Status

Accepted — ratified 2026-08-19 for backlog card `P7.F1.T1`.

This ADR supersedes the "future Wasmtime/WASI engine" clause in the
*Consequences* section of `plans/adrs/ADR-0019-wasm-plugin-runtime.md`. It
performs the supply-chain review that clause demanded, ratifies `wasmtime`
explicitly as the plugin runtime engine, and clears the debt recorded in
`plans/evidence/production/W0-truth-reconciliation/W0-7-wasmtime-adr-debt.md`.

ADR-0019 otherwise stands: it remains the authority for the plugin boundary,
manifest trust model, capability brokering, and quota semantics. This ADR
decides only *which engine* implements that boundary, and on what terms.

## Context

### The ordering that was lost

`P7.F1.T1` carries the stop condition "Stop if the runtime is added to the
workspace before the ADR is merged." The literal reading of that condition was
satisfied and its intent was not, and this ADR records both facts rather than
choosing the flattering one:

| Event | Commit | Date |
| --- | --- | --- |
| ADR-0019 merged | `10629c1` | 2026-05-25 |
| `wasmtime = "46.0.2"` added to the root `Cargo.toml` | `236a492` | 2026-07-01 |

*An* ADR did precede the dependency by five weeks. But ADR-0019 did not
authorize wasmtime — it explicitly withheld authorization:

> A future Wasmtime/WASI engine may be added only after supply-chain review and
> evidence proves equivalent no-ambient-authority behavior.

So the engine landed with the paperwork in the wrong state: an ADR existed, and
that ADR said "not yet." No supply-chain review was recorded at the time. The
sequence is auditable rather than quietly reordered, which is the whole point of
the stop condition; this ADR is the review that should have gated `236a492`.

### What already depends on the decision

`crates/legion-plugin/src/host.rs` implements `WasmPluginHost` on the wasmtime
core-module API, with four hostile `.wat` fixtures under `fixtures/hostile/`
exercising fuel exhaustion, memory growth, capability probing, and workspace
access. `crates/legion-plugin/src/wit_bindings.rs` generates the component-model
host bindings for the `legion:plugin/plugin-host` world (see ADR-0019 and
`P7.F1.T2`). Neither surface is reachable from a product binary today:
`legion-app` composes the metadata-only `PluginRuntimeHost`, not
`WasmPluginHost`. Ratifying the engine does not activate it.

## Decision

**Wasmtime is the Legion plugin runtime engine**, pinned at `46.0.2` in the
root `Cargo.toml` and consumed only by `legion-plugin`.

### Why wasmtime over the alternatives

- **Wasmtime** — the reference implementation of the component model and the
  canonical ABI. Legion's plugin ABI is defined in WIT
  (`crates/legion-plugin/wit/`), and `wasmtime::component::bindgen!` turns those
  files into host bindings at compile time, which makes ABI drift a build
  failure instead of a runtime mismatch. Ships fuel metering, epoch
  interruption, memory limits, and per-`Store` isolation — the exact primitives
  ADR-0019's quota model is written against. Bytecode Alliance governance and a
  published security policy with a defined embedder threat model.
- **Wasmer** — comparable performance, but component-model and WIT support lag
  the reference implementation, and Legion's ABI decision (`P7.F1.T2`) is
  component-model-shaped. Rejected on ABI grounds, not quality.
- **WasmEdge / WAMR** — strong in embedded and edge deployments where binary
  size dominates. Legion is a desktop IDE; the Rust embedding story and
  component-model maturity matter more than footprint. Rejected.
- **Writing an interpreter** — rejected. A hand-rolled sandbox is exactly the
  kind of security-critical wheel that ADR-0038's threat model says to buy, not
  build.

### Supply-chain review

This is the review ADR-0019 required. Findings as of 2026-08-19:

- **Provenance.** Bytecode Alliance project, Apache-2.0 WITH LLVM-exception.
  Compatible with the workspace license posture enforced by `deny.toml`.
- **Footprint.** `legion-plugin`'s full normal dependency closure is 162 crates;
  wasmtime and cranelift account for the large majority. This is the cost of the
  decision and it is accepted deliberately: the alternative is a smaller closure
  around a hand-written sandbox, which trades auditable third-party code for
  unauditable first-party code in the same trust position.
- **No WASI.** `wasmtime-wasi` is **not** a workspace dependency —
  `cargo tree -e normal -i wasmtime-wasi` reports
  `package ID specification 'wasmtime-wasi' did not match any packages`. There
  are therefore no WASI host bindings in the build to add to a linker, whether
  by accident or by a future careless edit. The `wasi`/`wasip2`/`wasip3` crates
  present in `Cargo.lock` are `getrandom`'s target-gated shims for
  `wasm32-wasi*` targets and are not compiled for any host target
  (`cargo tree -i wasip2` reports "nothing to print").
- **Advisories.** One accepted advisory touches a wasmtime path:
  `RUSTSEC-2026-0204` (crossbeam-epoch 0.9.18, invalid pointer dereference in
  the `fmt::Pointer` impl for `Atomic`/`Shared`). It is documented in
  `deny.toml` with the rationale that Legion's paths never format a null
  `Atomic`/`Shared`, and it is blocked upstream pending wasmtime accepting
  crossbeam-epoch `>=0.9.20`. No advisory is unique to this ratification.
- **Duplicate versions.** wasmtime's newer `toml`/`serde_spanned`/`wasm-encoder`
  chain (via `wasmtime-internal-cache` and `wast`/`wat`) is already recorded in
  the reviewed `deny.toml` `bans.skip` baseline. That baseline is version-pinned,
  so a future wasmtime bump that introduces a *new* duplicate still fails the
  gate and returns here for review.

### Conditions on the grant

The grant is narrow, and each condition below is machine-checked or
test-checked, not merely asserted:

1. **Only `legion-plugin` may declare `wasmtime`.** Enforced by
   `validate_plugin_runtime_dependency_gate` in `xtask`, run by
   `cargo run -p xtask -- check-deps`. Another crate taking the dependency is a
   gate failure.
2. **The dependency may not exist without this ADR.** The same gate requires
   `plans/adrs/ADR-0050-wasmtime-runtime-ratification.md` to be present and to
   name wasmtime whenever any workspace crate declares it. Deleting the ADR
   while keeping the engine fails `check-deps` — which is the stop condition
   from `P7.F1.T1` converted from a one-time instruction into a standing check.
3. **No ambient authority.** The core-module host
   (`WasmPluginHost::load_module`) rejects any import outside the single audited
   `env::host_log`, and rejects `wasi_snapshot_preview1` by name with a distinct
   `plugin_wasi_import_denied` code. The component-model host linker is
   populated *only* by `PluginHost::add_to_linker` from the generated bindings;
   `crates/legion-plugin/tests/wit_abi.rs::plugin_host_world_grants_no_ambient_authority`
   proves a component importing `wasi:cli/environment` fails to link against it.
4. **Activation stays separate.** This ADR ratifies a dependency. Reaching
   `WasmPluginHost` or the component host from a product composition root is a
   distinct decision gated by `P7.F1.T3` and onward, and by the runtime-surface
   activation gates in `plans/dependency-policy.md` §4.

## Consequences

- The debt in `W0-7-wasmtime-adr-debt.md` is cleared and `P7` is unblocked.
- `plans/dependency-policy.md` gains a plugin-runtime external-dependency entry
  alongside the Phase 8 rebaseline entries, so the policy now *admits* the
  engine instead of being silent about it.
- Legion inherits wasmtime's release cadence and advisory surface. Bumps are
  reviewed here and in `deny.toml`; the version-pinned `bans.skip` baseline
  guarantees a bump cannot silently widen the duplicate-crate surface.
- The 162-crate closure is a real cost to build times and audit scope. Accepted.
- If the component model is ever abandoned in favour of core modules only, or if
  Legion moves off wasmtime, this ADR is superseded rather than amended.

## References

- ADR-0019: WASM plugin runtime boundary (the boundary this engine implements)
- ADR-0038: OS sandbox layer (the kernel tier beneath the engine's sandbox)
- ADR-0046: Surface expansion freeze (no new crate; work lands in `legion-plugin`)
- `plans/evidence/production/W0-truth-reconciliation/W0-7-wasmtime-adr-debt.md`
- `plans/evidence/production/P7-F1/wasmtime-adr-and-wit-abi.md`
