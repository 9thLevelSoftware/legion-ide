# P7.F1.T3 / P7.F1.T4 Evidence — Plugin Quota Enforcement and Hostile Containment

Branch: `p7-f1-t3-t4-plugin-quota-containment`
Date: 2026-08-19
Readiness row: PR-VSC-001

## Tasks

**P7.F1.T3** — Enforce quotas, crash containment, capability denial, and audit
Acceptance: *"A fixture .wasm plugin loads, runs, and cannot escape permissions."*
Stop condition: *"Stop if a quota is allowed to be silently disabled per-plugin."*

**P7.F1.T4** — Add hostile-plugin tests: loop, OOM, capability probing, workspace access
Acceptance: *"Each hostile fixture is contained and the audit row records the attempt."*
Stop condition: *"Stop if a hostile fixture is allowlisted instead of denied."*

## What was already there, and why it did not hold

Both tasks were `in-progress` with green tests: `tests/quotas.rs` had 5 passing
tests and `tests/hostile.rs` had 4. The fixtures were real `.wasm` (assembled
from WAT by the `wat` crate and instantiated by Wasmtime), so the machinery was
genuine. The containment was not. Three things were wrong.

### 1. No quota was enforced at all except a host-call counter

`WasmPluginHost::default` built its engine from a bare `Config::new()`. Fuel
metering was off and epoch interruption was off, so `max_fuel` and
`max_wall_time_ms` were fields nobody read. No `ResourceLimiter` was installed,
so `max_memory_pages` was likewise inert. `max_output_bytes` and `max_events`
were unread. The one thing enforced, `max_host_calls`, counted *invocations*,
not host calls — there were no host calls to count, because the linker was
empty (`Linker::new`) and defined no host functions at all.

Measured before any change, with a probe test:

```
PROBE MEMORY: returned Ok(4096) (declared ceiling was 8 pages)
```

The guest grew linear memory to 4096 pages (256 MiB) against a declared ceiling
of 8 pages (512 KiB), and successfully stored a value at the 100 MiB offset.

```
running 1 test
PROBE: invoking infinite loop
error: test failed ... (exit code: 143)
```

`PROBE: returned` never printed. A true infinite loop ran until the 45-second
external timeout killed the process. `max_fuel: 1000` bought nothing.

### 2. The hostile fixtures did not perform the attacks they were named for

* `loop.wat` was not an infinite loop. It counted to 4 and then executed
  `unreachable`, trapping itself. The test asserting "the loop fixture is
  contained" was observing the fixture stop voluntarily.
* `oom.wat` asked for `2147483647` pages. That request exceeds the 4 GiB address
  space wasm32 allows, so **the WebAssembly specification** rejected it and
  returned -1 — the host contributed nothing. This is precisely the masking
  case: a host memory ceiling could be deleted without the test noticing.
* `capability_probe.wat` imported `env.host_log`, which the load-time import
  check *allowed*, but which the empty linker never defined. Instantiation
  failed with an unresolved-import error. The audit recorded `Crashed`, not
  `Denied`, and no capability was ever consulted. The `HostCallAccepted` audit
  variant was dead code, never constructed anywhere.

Only `workspace_access.wat` (a WASI `path_open` import, denied at load) was a
genuine denial.

### 3. The stop condition was violated: the manifest was the quota-disable switch

`load_fixture` read `manifest.quotas` and used those numbers directly.
`validate_plugin_manifest` in `legion-protocol` does not check quotas at all. A
plugin manifest is untrusted input, so a plugin could declare
`max_fuel: u64::MAX, max_memory_pages: u32::MAX, max_host_calls: u32::MAX` and
be granted exactly that. That is a per-plugin quota disable — it just happened
to be spelled as a field rather than a boolean.

## What this change does

### `legion-security`: a host-owned ceiling (`crates/legion-security/src/policy.rs`)

`PluginQuotaCeiling::grant(&declared) -> PluginQuotaGrant` returns the
*minimum* of the manifest's request and the host ceiling, per dimension, plus a
`PluginQuotaClamp` record for every dimension where the request lost.

The type deliberately has **no `enforced: bool`**. `BudgetCapPolicy` and
`RetentionExportPolicy` in the same file both have one, and for those surfaces
opt-in is correct. For plugin quotas it would be the stop condition, so the
switch does not exist. Because `grant` is a `min`, the only direction a manifest
can move a quota is down.

`PluginQuotaCeiling::HARD_MAX` is a second bound underneath the first:
`grant` mins against `min(self, HARD_MAX)`, so an operator-supplied or
policy-bundle-deserialized ceiling cannot widen the sandbox past what the crate
compiled in. The bounds on `HARD_MAX` itself are `const` assertions, so raising
them fails the build.

### `legion-plugin`: enforcement that the guest cannot cooperate with (`crates/legion-plugin/src/host.rs`)

* `Config::consume_fuel(true)` plus `store.set_fuel(granted.max_fuel)` — CPU.
* `Config::epoch_interruption(true)` plus a ticker thread holding an
  `EngineWeak` (so it exits with the host) and `set_epoch_deadline` derived from
  `max_wall_time_ms` — latency. The deadline is set immediately before entering
  the guest so compilation and instantiation do not consume the plugin's budget.
* `InvocationState: ResourceLimiter` refusing `memory.grow` past
  `max_memory_pages`, and writing a `QuotaExceeded` audit row naming the
  requested and granted page counts — memory.
* `env.host_log` is now actually implemented, and is the *entire* host surface.
  It checks, in order: the `plugin.event.emit` capability, the output-byte
  ceiling, the host-call counter, and the guest pointer's bounds. Each refusal
  records its own audit row and its own error code.

Capability denial happens at the **call** boundary, not at load, specifically so
that a probe produces an audit row recording the attempt — which is what T4's
acceptance asks for. A `refusal` field on the invocation state carries the real
cause out past the trap, so a capability denial is reported as
`plugin_capability_denied` rather than a generic `plugin_trapped`.

A plugin that breaks a quota or reaches for a capability is moved to `Disabled`
and refused on subsequent invocations. An ordinary `unreachable` trap is treated
as a bug rather than an attack: it yields `Crashed` and may retry.

### Fixtures (`crates/legion-plugin/fixtures/hostile/`)

Nothing is allowlisted. Every fixture performs its attack for real and is
refused by a named guard.

| Fixture | Attack | Contained by | Audit row |
| --- | --- | --- | --- |
| `infinite_loop.wat` | `(loop $spin br $spin)` — never exits | fuel quota | `Crashed`, plugin `Disabled` |
| `slow_loop.wat` | 200k iterations, cheap in fuel | wall-clock deadline | `Crashed` |
| `oom.wat` | grow to 4096 pages, write at 100 MiB | `ResourceLimiter` | `QuotaExceeded` / `Memory` |
| `capability_probe.wat` | call `env.host_log` without the capability | capability check | `Denied` naming call + capability |
| `host_call_flood.wat` | 4096 host calls, capability held | host-call counter | `QuotaExceeded` / `HostCall` |
| `oversized_output.wat` | 1024-byte payload against a 512-byte ceiling | output ceiling | `QuotaExceeded` / `Output` |
| `memory_escape.wat` | host-call pointer at 0x7FFF0000 | pointer bounds check | `Denied` |
| `workspace_access.wat` | WASI `path_open` | load-time import denial | `Denied`, never instantiated |
| `workspace_import_probe.wat` | `env.read_file` — same authority, non-WASI name | load-time import allowlist | `Denied` naming the import |

`workspace_import_probe.wat` exists because the previous rule could have been a
WASI blocklist. It proves the rule is an allowlist of exactly one function.

`oom.wat`'s request was lowered from 2^31 pages to 4096. 4096 pages is legal
wasm32, so the specification will not refuse it and only the host limiter can.

## Verification

```
$ cargo test -p legion-plugin -j 6
test result: ok. 14 passed  (lib)
test result: ok. 10 passed  (hostile)
test result: ok. 13 passed  (quotas)
test result: ok. 1 passed   (tampered)
test result: ok. 1 passed   (wit_abi)

$ cargo test -p legion-security -j 6
test result: ok. 95 passed  (lib)   [+ 12 integration suites, all green]

$ cargo clippy --workspace --all-targets -j 6 -- -D warnings
Finished `dev` profile

$ cargo run -p xtask -- check-deps
dependency policy checks passed
```

The whole hostile suite, including the infinite loop, finishes in under a tenth
of a second. Before this change the same loop did not finish at all.

## Mutation testing

Each guard was removed in turn, the matching test re-run, and the guard
restored. **17 mutations, 17 killed, 0 survived.**

| # | Mutation | Result |
| --- | --- | --- |
| M1 | `set_fuel(quotas.max_fuel)` → `set_fuel(u64::MAX)` | KILLED |
| M2 | `set_epoch_deadline(...)` deleted | KILLED |
| M2b | `set_epoch_deadline(u64::MAX)` | KILLED |
| M3 | `store.limiter(...)` deleted | KILLED |
| M4 | `memory_growing` always allows | KILLED |
| M5 | `host_log` capability check removed | KILLED |
| M6 | host-call counter check removed | KILLED |
| M7 | output-byte ceiling check removed | KILLED |
| M8 | guest pointer bounds check removed | KILLED |
| M9 | WASI import denial removed | KILLED |
| M10 | non-WASI import allowlist removed | KILLED |
| M11 | manifest quotas used instead of the clamped grant | KILLED |
| M11b | same, against the end-to-end memory test | KILLED |
| M12 | invocation (`max_events`) quota check removed | KILLED |
| M13 | clamp audit rows not written | KILLED |
| M14 | `HARD_MAX` clamp on a configured ceiling removed | KILLED |
| M15 | policy `grant` returns the declaration unclamped | KILLED |

### One mutation initially survived, and one masking pair was found

**M2 survived on the first run.** Deleting `store.set_epoch_deadline(...)`
entirely left `a_plugin_that_outlives_its_deadline_is_interrupted` passing.

The cause is a Wasmtime default. With `epoch_interruption(true)` enabled, a
store that never calls `set_epoch_deadline` starts with an *already expired*
deadline — `wasmtime-46.0.2/src/runtime/store.rs:1129` states that without
`set_epoch_deadline` "wasm will always immediately" trap. So the mutant trapped
the guest immediately, produced `Trap::Interrupt`, and satisfied the assertion.
The test proved a deadline existed; it did not prove the deadline came from the
declared quota.

Fixed by merging the two halves into one test,
`the_wall_clock_deadline_is_derived_from_the_declared_quota`, which asserts both
that a 0 ms budget is interrupted **and** that the same module with the same
fuel and a 2000 ms budget returns 200000 normally. M2 now fails on the second
half; M2b (deadline made effectively infinite) fails on the first.

**Two more mutations were killed only by the specific error code being
asserted**, which showed the guards were not isolated:

* M6 (host-call counter removed) originally produced
  `plugin_fuel_quota_exceeded` — the flood ran out of fuel instead. Fixed by
  raising `max_fuel` to 5,000,000 in the flood tests, so fuel cannot stand in
  for the counter. M6 now fails on `expect_err` outright.
* M7 (output ceiling removed) originally produced
  `plugin_host_call_out_of_bounds` — the pointer check caught the 16 MiB
  payload. Fixed by shrinking `oversized_output.wat`'s payload to 1024 bytes,
  which is *inside* the fixture's one page of memory, so only the output quota
  can refuse it.

M1 remains a defense-in-depth case: with fuel disabled, the wall-clock deadline
still stops the infinite loop after 50 ms. The mutation is killed because the
test asserts `plugin_fuel_quota_exceeded` exactly. This is two independent
guards over one attack, not a vacuous test, and it is deliberate — but it is
recorded here so a future reader does not mistake the coverage for isolation.

## Stop conditions

**T3 — "Stop if a quota is allowed to be silently disabled per-plugin."**
The only per-plugin quota surface is the manifest, and it is clamped by `min`.
`a_manifest_asking_for_unlimited_quotas_is_granted_the_host_ceiling` checks all
seven dimensions against the ceiling;
`a_manifest_declaring_unlimited_fuel_is_still_stopped_mid_loop` and
`a_manifest_declaring_unlimited_memory_is_still_held_to_the_page_ceiling` check
the same end-to-end with a running guest. No clamp is silent:
`a_clamped_quota_is_recorded_in_the_audit_rather_than_applied_silently`
requires one `QuotaClamped` audit row per clamped dimension. `PluginQuotaCeiling`
has no disable flag and no per-plugin setter, and 0 is 0 rather than a sentinel
for unlimited (`a_zero_quota_is_zero_and_never_means_unlimited`).

**T4 — "Stop if a hostile fixture is allowlisted instead of denied."**
No fixture is allowlisted, skipped, or `#[ignore]`d. Each of the nine performs
its attack against a live Wasmtime instance and is refused by a specific named
guard, and each test asserts the exact error code so that a different guard
firing cannot silently satisfy it.

## Not done

* `max_storage_bytes` is clamped by the ceiling but not enforced at a call site,
  because the sandboxed host exposes no storage host call to enforce it at. The
  clamp is in place for when one is added.
* Wall-clock enforcement is tested deterministically at the 0 ms boundary and
  with a generous budget. A mid-range wall-clock assertion would be timing
  dependent, so none was written.
