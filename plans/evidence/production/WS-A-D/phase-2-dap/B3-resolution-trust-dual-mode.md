# Phase 2 B3 — Adapter resolution, trust, dual-mode honesty

**Date:** 2026-07-21

## Delivered

| Item | Location |
| --- | --- |
| Adapter resolution | `crates/legion-debug/src/adapter_resolve.rs` (`LEGION_DAP_MODE`, `LEGION_DAP_ADAPTER`, `LEGION_DAP_USE_FAKE`) |
| Wire honesty | No PATH auto-discovery of Microsoft DAP adapters; framing is Legion provisional JSON-RPC |
| Trust deny | `DebugWorkflow::launch` — untrusted → `debug.adapter.launch denied` |
| Live one-shot launch | App uses `LiveDapSession` when adapter resolves; else fixture (`auto`) |
| Live fail-closed | `LEGION_DAP_MODE=live` → no fixture fallback on missing adapter or spawn failure |
| Per-source breakpoints | Live launch groups `setBreakpoints` by path |
| Program path | Relative `program_label` joined to configuration `cwd` |
| Projection flag | `DebugProjection.live_adapter` |
| Desktop dual banner | `DEBUG_LIVE_BANNER` vs `DEBUG_SIMULATED_BANNER` |
| Tests | untrusted deny + live fake path in `debug_workflow` app tests |
| USER_GUIDE | product-areas note updated |

## Env (operators)

| Variable | Meaning |
| --- | --- |
| `LEGION_DAP_MODE` | `fixture` \| `live` \| `auto` (default). `live` fails closed. |
| `LEGION_DAP_ADAPTER` | Absolute path to a **Legion-compatible** adapter (provisional JSON-RPC envelope) |
| `LEGION_DAP_USE_FAKE` | `1` — allow in-tree `fake_dap_adapter` for CI/dev |

## Explicitly still open

- Microsoft DAP message codec (`seq`/`type`/`command`) + contract test vs real adapter
- PATH auto-discovery of `lldb-dap` / CodeLLDB (blocked on codec)
- Pre-launch `cargo build` from `cargo_args` for product binaries
- Persistent live session for step/continue after launch (one-shot live then disconnect today)
- Documented CodeLLDB install UX polish
- Sandbox wrap of adapter spawn (Phase 3)

## Verification

```text
cargo test -p legion-debug --all-targets
cargo test -p legion-app --test debug_workflow
cargo test -p legion-desktop --test debug_workflow
cargo test -p legion-ui --test debug_projection
```

---

# P2.F3.T2 — Policy-gated adapter binary resolution

**Date:** 2026-08-16
**Task:** `P2.F3.T2` — "Wire CodeLLDB for Rust with policy-gated adapter resolution"
**Acceptance (binding):** "Adapter binary resolution is policy-gated; untrusted workspaces cannot launch debug adapters."
**Stop condition (binding):** "Do not add a 'trust all adapters' switch."
**Backlog status after this change:** `in-progress`, deliberately. The acceptance sentence
is met and tested; the card's title names CodeLLDB, and no CodeLLDB or `lldb-dap` binary
has been run for it. See *Not claimed*.

## Two findings, stated plainly

Both were found by reading the code this task pointed at, and both are the reason the task
was not already closable.

1. **`LEGION_DAP_ADAPTER` would spawn any file it named. That is a security gap, not a
   tidiness issue.** Resolution checked only that the path existed. A workspace-influenced
   or mistyped value — a script dropped in a repo, anything on `PATH` under an adapter's
   name — became a child process holding the debuggee's authority, on a path whose only
   other gate was an app-level trust `if` that nothing in `legion-debug` could see. The
   allowlist added here is the fix: env chooses *where* an adapter is, policy chooses
   *what* may run.
2. **`DebugAdapterLaunchPolicy` had a green unit test and no caller.** It existed in
   `legion-security` from B3 with `allows_resolution`, a passing test, and no production
   code path — it was not even a field of `SecurityPolicy`. The test proved the function
   returned the right booleans; nothing proved the function was ever consulted. This is
   precisely the failure mode this repo's `claim-audit` gate exists to catch, one level
   below documentation: a passing test standing in for a shipped behavior. It is now
   load-bearing — the broker's `debug.` branch calls it, and removing the call fails tests.

## What was already true before this change

| Claim | State on 2026-08-15 |
| --- | --- |
| Untrusted workspaces cannot launch adapters | **True.** `DebugWorkflow::launch` compared `context.trust != Trusted` inline and denied. Covered by `debug_workflow_denies_launch_on_untrusted_workspace`. |
| Resolution finds CodeLLDB | **Partly.** `codelldb` was already a PATH candidate name alongside `lldb-dap` / `lldb-vscode`, preferred-name-first. No CodeLLDB-specific transport work existed, and still does not (see *Not claimed*). |
| Resolution is policy-gated | **False.** `resolve_live_adapter` / `resolve_system_adapter` consulted only `LEGION_DAP_MODE`, `LEGION_DAP_ADAPTER`, `LEGION_DAP_USE_FAKE`. Any existing file named by `LEGION_DAP_ADAPTER` was returned and spawned. |
| A debug adapter policy exists | **Existed but was dead.** `legion_security::DebugAdapterLaunchPolicy` (with `allows_resolution`) and a passing unit test existed since B3, but **no production code called it** and it was not a field of `SecurityPolicy`. A passing test on an uncalled policy is the exact shape of an overclaim; it is now load-bearing. |
| `debug.adapter.launch` capability | **Label only.** The denial message named the capability, but the broker was never asked and `DenyByDefaultBroker` had no `debug.` branch (it fell through to deny-by-default). |
| Fixture mode is the default | **False, already.** `LEGION_DAP_MODE` has defaulted to `auto` (try live, fall back to fixture) — there was no fixture default left to flip. |
| Launch config synthesis from cargo | **Already true.** `legion_project::discover_cargo_debug_configurations` synthesizes configs from `Cargo.toml` (`src/main.rs` + `[[bin]]`) with `stop_on_entry` and `cargo build` prebuild args. |

## What changed

| Change | Location |
| --- | --- |
| Pure move of the debug workflow out of the merge chokepoint (no behavior change, own commit) | `crates/legion-app/src/lib.rs` → `crates/legion-app/src/debug_workflow.rs` (1,128 lines) |
| `DebugAdapterLaunchPolicy` moved into the policy module and given an adapter-binary allowlist | `crates/legion-security/src/policy.rs` |
| Policy added to the org policy bundle (`#[serde(default)]`, back-compatible) | `SecurityPolicy::debug_adapter_policy` |
| `debug.` branch in the broker: trust gate, empty-allowlist deny, binary allowlist, unknown-subcommand deny-by-default | `crates/legion-security/src/lib.rs` |
| `debug.` added to the unknown-trust deny list | `requires_trusted_workspace_for_request` |
| `AdapterResolutionGrant` — resolution now requires a granted `debug.adapter.launch` decision plus a non-empty allowlist | `crates/legion-debug/src/adapter_resolve.rs` |
| `resolve_live_adapter` / `resolve_system_adapter` take the grant and filter every hit, including `LEGION_DAP_ADAPTER` and the CI fake | same |
| App asks the broker before resolving, and mints the grant from the decision | `crates/legion-app/src/debug_workflow.rs` |

The stop condition is respected: widening is done by **naming binaries**, and an empty
allowlist denies rather than allows. There is no boolean that turns the check off.

### Behavior change for operators

`LEGION_DAP_USE_FAKE=1` alone no longer yields a live fake adapter in the product path:
`fake_dap_adapter` is not in the shipped allowlist, and the env var deliberately did not
become a policy bypass — that would be the "trust all adapters" switch under another name.
The in-tree fake is still reachable from tests, which widen the allowlist explicitly. No
CI workflow or script in this repo sets `LEGION_DAP_USE_FAKE`, so nothing in the pipeline
depended on the old behavior.

## Tests

| Test | Crate | Asserts |
| --- | --- | --- |
| `policy::tests::debug_adapter_policy_allowlists_known_adapters_only` | legion-security | `lldb-dap` / `codelldb` allowed (case-insensitive); `bash`, `fake_dap_adapter` denied |
| `policy::tests::debug_adapter_policy_empty_allowlist_denies_every_binary` | legion-security | empty list is not a vacuous allow |
| `policy::tests::debug_adapter_policy_rejects_blank_binary_names` | legion-security | blank entries authorize nothing |
| `tests::debug_adapter_launch_policy_requires_trusted_workspace_by_default` | legion-security | pre-existing; still passes |
| `tests::debug_adapter_launch_is_brokered_and_denied_for_untrusted_workspaces` | legion-security | Untrusted **and** Unknown trust denied; Trusted allowed |
| `tests::debug_adapter_launch_denies_non_allowlisted_binary_and_unknown_subcommands` | legion-security | `command_binary=bash` denied; `debug.adapter.attach` deny-by-default |
| `tests::debug_adapter_launch_denied_when_policy_allowlist_is_empty` | legion-security | fail-closed policy |
| `adapter_resolve::tests::grant_requires_a_granted_debug_adapter_launch_decision` | legion-debug | denied decision, wrong capability, empty/blank allowlist all mint no grant |
| `adapter_resolve::tests::grant_permits_only_allowlisted_binaries` | legion-debug | `/bin/sh`, `fake_dap_adapter.exe` refused |
| `explicit_adapter_path_is_refused_unless_the_binary_is_allowlisted` | legion-debug (`tests/adapter_resolution_policy.rs`) | **negative + positive control**: `LEGION_DAP_ADAPTER` pointed at a real, existing, non-adapter executable resolves to `None`; the same path with the binary allowlisted resolves to `Some` |
| `debug_workflow::adapter_policy_tests::denied_decision_mints_no_resolution_grant` | legion-app | a denied decision cannot reach resolution |
| `debug_workflow::adapter_policy_tests::shipped_policy_allows_rust_adapters_but_not_the_ci_fake` | legion-app | the shipped default cannot launch the in-tree fake |
| `debug_workflow::adapter_policy_tests::live_fake_test_seam_widens_policy_rather_than_bypassing_it` | legion-app | the fake seam adds to the allowlist instead of skipping it |
| `debug_workflow::adapter_policy_tests::capability_id_matches_between_security_policy_and_debug_crate` | legion-app | the two crates' capability id constants agree (they cannot depend on each other) |
| `debug_workflow_denies_launch_on_untrusted_workspace` | legion-app | pre-existing; now backed by a real broker decision |

Counts: legion-security lib `75 passed`; legion-debug `19 passed` across all targets;
legion-app `debug_workflow` integration `6 passed`, new lib unit tests `4 passed`.

## Not claimed

Debugger work is easy to overclaim. The following are **not** established by this change:

1. **No real CodeLLDB or `lldb-dap` run happened on this machine.** No adapter binary is
   installed here (`lldb-dap`, `codelldb`, `lldb-vscode` all absent from `PATH`).
   `system_adapter_dogfood` and `system_adapter_launch_step_dogfood` report `ok`, but they
   took the **soft-skip** branch. **A clean skip is not proof.** Only
   `LEGION_DAP_DOGFOOD=1` makes those tests fail closed, and that run has not been done
   here. The last recorded real-adapter run is B13, not this change.
2. **CodeLLDB is not verified as a working transport.** `codelldb` is an allowlisted and
   PATH-searched name, and would be spawned on stdio like `lldb-dap`. Whether the
   `codelldb` binary speaks Microsoft DAP over stdio without a `--port` argument has not
   been verified against a real installation. Treat "CodeLLDB wiring" as *resolution and
   policy*, not as a demonstrated debug session.
3. **The grant is not unforgeable.** `AdapterResolutionGrant` prevents resolution
   *without* a decision; it does not defend against code in the same process constructing
   a granted `CapabilityDecision`. The enforcement boundary is the app calling the broker.
4. **The allowlist is not yet operator-editable through the settings UI.** It lives in
   `SecurityPolicy` and deserializes from a policy bundle, but no settings surface writes it.
5. **No default was flipped off fixture mode.** `auto` was already the default and remains
   it; with no adapter installed the product still lands on the simulated fixture, and the
   projection still says so.
6. **Config synthesis was not extended.** See the recorded gap below.

## Known gap recorded, not fixed: cargo launch-config discovery

`legion_project::discover_cargo_debug_configurations` (`crates/legion-project/src/lib.rs`)
is the "zero-config Rust debug" surface. It reads `Cargo.toml` as **text** — it does not
invoke `cargo metadata` — and builds configurations from exactly two sources:

- the package name, when `src/main.rs` exists, and
- explicit `[[bin]] name = …` entries.

Not discovered, and therefore not debuggable without a hand-written configuration:

| Missed | Why |
| --- | --- |
| `src/bin/*.rs` autobins | Cargo infers these; there is no `[[bin]]` stanza to parse |
| Workspace members | Only the root manifest is read; `[workspace] members` is ignored, and a virtual manifest has no `[package] name`, so it errors with `ManifestParse` |
| `[[example]]` / benches / test binaries | Never enumerated |
| `required-features`, renamed targets, `[[bin]] path` without `name` | Not modeled |

**Decision: recorded, not fixed.** The fix is small but it is a different task — it belongs
to zero-config debug UX, not to policy-gated resolution. It needs its own tests (autobin
directory listing, virtual-manifest workspace, a member with no `src/main.rs`) and its own
evidence, and `crates/legion-project` is outside this task's `files` and acceptance.
Fixing it here would have meant shipping untested discovery behavior under a security
task's evidence, which is the substitution this evidence file is arguing against.
Recommend a follow-up task under P2.F3 that replaces the text parse with `cargo metadata`.

## Verification

```text
cargo fmt --all
cargo test --workspace --all-targets --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- verify-kanban-backlog
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- claim-audit
cargo run -p xtask -- verify-readiness-consistency
```
