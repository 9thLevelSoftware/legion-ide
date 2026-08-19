# P7.F1.T1 / P7.F1.T2 — Wasmtime ratification and a WIT ABI that actually generates bindings

Date: 2026-08-19
Cards: `P7.F1.T1` (ADR/dependency-policy for the runtime), `P7.F1.T2` (minimal WIT ABI)
Readiness row: PR-VSC-001

## Summary of what was actually wrong

Both cards looked closer to done than they were.

- **T1** — `plans/adrs/ADR-0019-wasm-plugin-runtime.md` existed and `wasmtime`
  was already a workspace dependency, but the ADR *withheld* authorization and
  `plans/dependency-policy.md` never named the engine. The gap was real.
- **T2** — the three `.wit` files existed and read plausibly, but
  `crates/legion-plugin/src/wit_bindings.rs` was **never compiled**.
  `crates/legion-plugin/src/lib.rs` declared only `pub mod host;` and
  `pub mod registry;`. The file's single line was `wit_bindgen::generate!();`,
  invoking a *guest*-side macro from a crate that is not a dependency of this
  workspace at all. Had the module ever been declared it would not have built.
  `crates/legion-plugin/src/manifest.rs` was undeclared for the same reason, so
  `plugin_manifest_permission_review_rows` was dead code and its test never ran.
  The only test guarding the ABI, `tests/wit_abi.rs`, did nothing but
  `assert!(grammars.contains("interface grammars"))` — string-grepping files it
  never compiled. **No host bindings were generated, so T2's acceptance was not
  met however good the `.wit` files looked.**

## T1 — the ordering finding, stated plainly

The stop condition is "Stop if the runtime is added to the workspace before the
ADR is merged."

| Event | Commit | Date |
| --- | --- | --- |
| ADR-0019 merged | `10629c1` | 2026-05-25 |
| `wasmtime = "46.0.2"` added to root `Cargo.toml` | `236a492` | 2026-07-01 |

```
$ git log --diff-filter=A --format="%H %ad %s" --date=short -- plans/adrs/ADR-0019-wasm-plugin-runtime.md
b130d5baca901605acb3c3d1291a805c35b328bb 2026-08-12 adding docs back
10629c1f5b9cc4c3c1ac32501fbd12c74048e4e7 2026-05-25 feat(plugin): introduce WASM plugin runtime with manifest validation and host call management

$ git log -S "wasmtime" --format="%H %ad %s" --date=short -- Cargo.toml
236a492921b988a2b135c8238372ba4aa84b7fdc 2026-07-01 feat: advance Legion productionization surfaces
```

**The letter was honoured; the intent was not.** An ADR did precede the
dependency by five weeks — so read literally, the condition held. But ADR-0019
did not authorize wasmtime. Its *Consequences* section said:

> A future Wasmtime/WASI engine may be added only after supply-chain review and
> evidence proves equivalent no-ambient-authority behavior.

The engine therefore landed against an ADR that said "not yet", with no
supply-chain review recorded. This is the same conclusion
`plans/evidence/production/W0-truth-reconciliation/W0-7-wasmtime-adr-debt.md`
reached on 2026-08-05; that note called the condition "already violated", which
is right about the intent and imprecise about the letter. Both readings are now
recorded rather than the flattering one.

### What closes it

1. `plans/adrs/ADR-0050-wasmtime-runtime-ratification.md` — supersedes the
   ADR-0019 clause, performs the supply-chain review (provenance, 162-crate
   closure, the *absence* of `wasmtime-wasi`, the one accepted advisory on a
   wasmtime path, the duplicate-version baseline), compares the rejected
   alternatives, and attaches four conditions to the grant.
2. `plans/dependency-policy.md` — the `legion-plugin` section now admits the
   engine by name and refuses `wasmtime-wasi`.
3. `validate_plugin_runtime_dependency_gate` in `xtask/src/main.rs`, wired into
   `cargo run -p xtask -- check-deps`. **This converts the one-time stop
   condition into a standing check**: while any workspace crate declares
   `wasmtime`, the ADR must exist and name it, the policy clause must be
   present, and no crate other than `legion-plugin` may declare it. The gate is
   deliberately silent when no crate declares the engine — a policy entry for a
   dependency nobody has is not a violation.

## T2 — the WIT ABI now compiles into real host bindings

`crates/legion-plugin/src/wit_bindings.rs` now runs
`wasmtime::component::bindgen!({ path: "wit", world: "plugin-host" })`, and
`lib.rs` declares `pub mod wit_bindings;` and `pub mod manifest;`. The generated
surface, confirmed by compiling code against it:

- `wit_bindings::legion::plugin::grammars::{Host, GrammarContribution}`
- `wit_bindings::legion::plugin::themes::{Host, ThemeContribution}`
- `wit_bindings::legion::plugin::lsp::{Host, LspAdapterContribution}`
- `wit_bindings::PluginHost::{add_to_linker, instantiate, call_activate}`

`world plugin-host` gained `export activate: func();`. Without it the world had
no host-callable entrypoint at all: a component could import the registration
interfaces but nothing could ever trigger registration. This is the minimum
addition that makes the ABI usable, and it is what `call_activate` is generated
from.

### The anti-drift anchor the stop condition asks for

`crates/legion-plugin/fixtures/abi/contributions.wat` is a **hand-written
WebAssembly component** targeting `legion:plugin/plugin-host`. It is written
directly against the canonical ABI — each `string` flattens to a (pointer,
length) i32 pair, so a 5-field record lowers to 10 core parameters — and is not
generated from the `.wit` files. The two are independent statements of the same
ABI, so drift in either one breaks the link.

`crates/legion-plugin/tests/wit_abi.rs` instantiates that component against the
generated bindings, calls `activate`, and asserts every field of all three
records arrives verbatim. It also proves containment: a probe component
importing `wasi:cli/environment@0.2.0` must fail to link against a linker
populated only by `PluginHost::add_to_linker`.

```
$ cargo test -p legion-plugin -j 6
     Running unittests src\lib.rs
running 15 tests
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests\hostile.rs
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests\quotas.rs
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests\tampered.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests\wit_abi.rs
running 5 tests
test wit_world_declares_all_three_contribution_interfaces ... ok
test plugin_host_world_grants_no_ambient_authority ... ok
test lsp_adapter_abi_carries_every_declared_field_across_the_boundary ... ok
test theme_abi_carries_every_declared_field_across_the_boundary ... ok
test grammar_abi_carries_every_declared_field_across_the_boundary ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The lib count went 14 → 15 because `manifest::tests::plugin_manifest_permission_review_rows_are_structured`
runs for the first time:

```
$ cargo test -p legion-plugin --lib -- --list
manifest::tests::plugin_manifest_permission_review_rows_are_structured: test
tests::plugin_manifest_incompatible_abi_is_rejected_before_load: test
...
```

## Mutation testing

A test that passes whether or not the feature works is worthless, so every new
assertion was broken on purpose. **Nine mutations, nine kills.** Working tree
verified clean (`git status --short`) after each restore.

| # | Mutation | Result |
| --- | --- | --- |
| M1 | Swap `grammar-name` / `artifact-uri` in `grammars.wit` | **Killed 3 tests.** `component imports instance 'legion:plugin/grammars', but a matching implementation was not found in the linker` / `instance export 'register-grammar' has the wrong type` |
| M2 | Delete `required-capability` from `themes.wit` | **Killed at compile time.** `error[E0609]: no field 'required_capability' on type '&ThemeContribution'` |
| M3 | Delete `export activate: func();` from the world | **Killed at compile time.** `error[E0599]: no method named 'call_activate' found for struct 'PluginHost'` |
| M4 | Change fixture data `rust-analyzer` → `clangd-------` (same length) | **Killed 1 test.** `assertion 'left == right' failed  left: "clangd-------"  right: "rust-analyzer"` |
| M5 | Add `linker.define_unknown_imports_as_traps(&component)` — the permissive-linker mistake | **Killed the containment test.** `a component importing WASI must not link: Instance { .. }` |
| M6 | Delete the wasmtime clause from `plans/dependency-policy.md` | **Killed `check-deps`.** ``- `plans/dependency-policy.md` must admit runtime engine `wasmtime` with clause `WASM plugin runtime engine (`legion-plugin`): `wasmtime``` `` |
| M7 | Remove `ADR-0050` while the engine stays in the workspace | **Killed `check-deps`.** ``- runtime engine `wasmtime` is in the workspace but `plans/adrs/ADR-0050-wasmtime-runtime-ratification.md` is missing; the ADR must be merged before the runtime`` |
| M8 | Add `wasmtime` to `crates/legion-vscode-compat/Cargo.toml` | **Killed `check-deps`.** ``- workspace package `legion-vscode-compat` must not declare runtime engine `wasmtime`; only legion-plugin may`` |
| M9 | Make `validate_plugin_runtime_dependency_gate` return `Vec::new()` unconditionally | **Killed the xtask unit test.** `missing ADR should be reported, got: []` |

M1 and M4 are the two that matter most for the T2 stop condition: M1 proves the
fixture pins the ABI *shape*, M4 proves the guest's bytes actually traverse the
canonical ABI into the host rather than the host inventing them. M5 proves the
containment assertion is not a tautology. M6–M8 prove T1's policy is enforced
rather than prose, and M9 proves the enforcement is not itself vacuous.

Note on M7: it also fails the xtask unit test, which reads the real ADR from
disk. That is intended — the gate and its test both refuse to pass without the
ratifying document.

## Gates

```
$ cargo run -p xtask -- check-deps
dependency policy checks passed

$ cargo run -p xtask -- docs-hygiene
documentation hygiene checks passed

$ cargo run -p xtask -- claim-audit
claim audit passed

$ cargo clippy --workspace --all-targets -j 6 -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 24s
```

`cargo fmt --all` applied.

## Scope explicitly NOT taken

Ratifying the engine is not activating it. `legion-app` still composes the
metadata-only `PluginRuntimeHost`; neither `WasmPluginHost` nor the
component-model host is reachable from a product binary. Wiring either one is
`P7.F1.T3` and onward, and remains gated by
`plans/dependency-policy.md` §4. No new workspace crate was created (ADR-0046).
