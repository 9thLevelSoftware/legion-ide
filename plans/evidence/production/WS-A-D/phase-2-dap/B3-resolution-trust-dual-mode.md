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

---

# P2.F3.T2 — Making the skip falsifiable: CI adapter inventory + fail-closed dogfood

**Date:** 2026-08-17
**Task:** `P2.F3.T2` — "Wire CodeLLDB for Rust with policy-gated adapter resolution"
**Backlog status after this change:** still `in-progress`. Read *Not claimed* before
reading anything else here.

## The problem this addresses

The 2026-08-16 section above records the honest reason the card stayed open: both
system-adapter dogfood tests report `ok` on every machine, through their **soft-skip**
branch, because no `lldb-dap` / `lldb-vscode` / `codelldb` binary has ever been present
when they ran. A clean skip is not proof. Closing the card needs a real adapter, and the
developer machine does not have one.

The opening is that a CI runner might. A test that asserted
`resolve_system_adapter(...).is_none()` failed on `windows-latest`, which means the gate
correctly refused a non-allowlisted explicit path and then found a genuinely allowlisted
adapter on `PATH`. That is one data point on one image, and it was an accident. This
change turns it into something deliberate and repeatable.

## What was built

| Item | Location |
| --- | --- |
| Adapter inventory command | `xtask/src/dap_adapter_probe.rs`, `cargo run -p xtask -- dap-adapter-probe` |
| CLI wiring | `xtask/src/main.rs` (`DapAdapterProbe`, `run_dap_adapter_probe_command`) |
| CI workflow | `.github/workflows/legion-dap-dogfood.yml` (`inventory` + `dogfood` jobs, 3 OSes) |

`dap-adapter-probe` searches `PATH` for exactly the names the resolver searches for,
records where each was found, whether the shipped allowlist would accept its stem, and its
`--version` banner, then writes `target/dap-adapter/probe_report.toml`. It also reports
**versioned variants** (`lldb-dap-18`) separately, because those are present-but-unreachable
and the difference matters. It is report-only unless `--require` is passed.

Flags: `--provenance shipped|installed|unknown` (recorded verbatim in the report),
`--require` (exit 1 when nothing the resolver could return is present), `--no-versions`,
`--out`.

## The two jobs, and why they are two

`inventory` runs **before anything is installed** and never fails. It answers "what does
this runner image ship". `dogfood` provisions an adapter when the image has none and then
runs the dogfood tests with `LEGION_DAP_DOGFOOD=1`. Merging them would produce a report
that cannot distinguish *the platform ships a debugger* from *we installed a debugger*,
which are different claims about what a user gets out of the box. The `provenance` field
carries the distinction into the artifact; the provisioning steps set it from which branch
they actually took, not from which OS they are on.

## The gate was not weakened

The adapter allowlist is untouched. No "trust all adapters" switch, no env override of
policy, no new allowlist entry, no test seam in `legion-debug` at all.

The one place this came close is Linux: `apt-get install lldb` leaves a **versioned**
`lldb-dap-18`, which the resolver does not search for and whose stem the allowlist does not
accept. The fix taken is a symlink exposing that binary under the exact filename
`lldb-dap`, which is already allowlisted. That changes a *filename*, not a trust decision —
a binary that is not an allowlisted adapter is no more launchable after this than before.
The alternative fix (teach the resolver to search versioned names, and the allowlist to
accept versioned stems) is a real policy widening and was deliberately not done here.

The `dogfood` job also runs `adapter_resolution_policy` as a control, because that test's
negative case — a non-allowlisted explicit `LEGION_DAP_ADAPTER` path is refused — is only
fully meaningful on a machine that *has* a real adapter to fall through to.

## What ran locally, and what it proves

Local runs on the developer Windows machine, which has **no debug adapter**:

| Command | Result |
| --- | --- |
| `cargo run -p xtask -- dap-adapter-probe --provenance shipped` | exit 0; `resolvable_adapter_count=0`; report written |
| `cargo run -p xtask -- dap-adapter-probe --require` | exit 1, with the "install one" message |
| same, with a synthetic `lldb-dap.exe` and `lldb-dap-18.exe` on `PATH` | exit 0; adapter found with `allowlisted_stem=true` and a `--version` banner; the versioned file reported as a non-resolvable variant |
| `cargo test -p xtask --lib dap_adapter_probe` | 6 passed |

The synthetic binary was a copy of `xtask.exe` renamed. It proves the **probe's** found-path,
version capture, variant detection and TOML escaping work. It proves nothing whatsoever
about a debugger, and it is not on any code path a dogfood test uses.

The six unit tests are machine-independent by construction — they operate on synthetic
paths and rendered strings, never on what this machine happens to have installed. That is
deliberate: `explicit_adapter_path_is_refused_unless_the_binary_is_allowlisted` broke CI by
quietly requiring that *no* adapter be installed, and the mirror-image bug (requiring one to
be present) would break every developer machine instead.

One of the six is a drift guard. `xtask` may not depend on `legion-debug`
(`plans/dependency-policy.md`), so `PROBE_NAMES` is a copy of the resolver's alias list;
`probe_names_match_the_resolver_alias_list` reads
`crates/legion-debug/src/adapter_resolve.rs` and fails if the two diverge. Without it, a new
alias in the resolver would make the probe silently under-report — reintroducing exactly the
"looks fine, proves nothing" failure this work exists to remove.

## Not claimed

**Nothing here has yet run a debug adapter.** This section must be re-read after the first
CI run; until then every row below is still open.

1. **No CI run has happened.** The workflow is code, not evidence. No adapter binary has
   been resolved, spawned, or handshaken by this change on any machine. The developer
   machine still has no debugger and the local test suite still takes the soft-skip branch.
2. **What each runner ships is still unknown.** The `windows-latest` LLVM data point is an
   inference from one failed assertion, not an inventory. `ubuntu-latest` and `macos-latest`
   are unmeasured. Producing that inventory is what the `inventory` job is for, and it has
   not run.
3. **Whether the dogfood tests pass under `LEGION_DAP_DOGFOOD=1` is unknown on all three
   platforms.** The `system_adapter_dogfood` doc comment already records that some runners
   ship an `lldb-dap.exe` without a working LLDB runtime; a fail-closed run is exactly what
   would expose that, and it may well be what the first run reports on Windows.
4. **Three different things are being kept apart here and must not be collapsed:**
   - *resolution* — the resolver returns a policy-permitted path. Tested, including
     negative cases; does not require a real adapter.
   - *launch* — that binary spawns and completes a Microsoft-DAP `initialize`.
     `system_adapter_dogfood` covers this and has never run against a real adapter.
   - *a working debug session* — breakpoints, launch, stop, step.
     `system_adapter_launch_step_dogfood` covers this and has never run against a real
     adapter either.

   None of the three is *zero-config Rust debugging*, which is what roadmap item 1.9 names.
   That additionally requires launch-configuration discovery, and the 2026-08-16 section
   above records `discover_cargo_debug_configurations` as a text parse that misses autobins,
   workspace members and examples. No Legion UI is involved anywhere in this workflow.
5. **The `apt` finding is a real product gap, recorded not fixed.** A Linux user who runs
   `apt install lldb` gets a working debug adapter that Legion will not find, because the
   resolver searches exact names and the allowlist matches exact stems. The CI workflow
   sidesteps this with a symlink; a user has no symlink. Fixing it properly is two coupled
   decisions — which names to search, and which stems policy accepts — and belongs in its
   own task with its own tests, not smuggled in under this one.
6. **CodeLLDB specifically is still unverified.** `codelldb` remains an allowlisted,
   `PATH`-searched name. None of the provisioning steps install CodeLLDB (they install LLVM,
   which provides `lldb-dap`), and whether the `codelldb` binary speaks Microsoft DAP over
   stdio without a `--port` argument is still untested. The card's title says CodeLLDB; the
   evidence, if the first run is green, will say `lldb-dap`.

## What a maintainer should look for in the first run

1. **`inventory` job, all three OSes** — read `dap-adapter-inventory-<os>` and the step log.
   The answer to "what does each runner have" is `resolvable_adapter_count` plus the
   `[[adapter]]` and `[[variant]]` tables. Expect this job to be green regardless of what it
   finds; a green `inventory` with `resolvable_adapter_count = 0` is a *finding*, not a pass.
2. **`dogfood` job, provisioning step** — which branch it took. The log line is either
   "image already provides lldb-dap at …" or an install. Cross-check against
   `LEGION_DAP_PROVENANCE` in the uploaded `dap-adapter-dogfood-probe-<os>` report. If those
   disagree, the report is wrong and must be fixed before its numbers are quoted anywhere.
3. **A red `Probe for DAP adapters (post-provisioning, required)` step** means provisioning
   failed, not that the debugger failed. Its error names what was searched.
4. **A red `Dogfood — initialize handshake` step** is the interesting failure: an adapter
   resolved and spawned but did not complete `initialize`. That is the broken-LLVM-install
   case, and it is a genuine result about that platform.
5. **A red `Dogfood — launch and step`** with the handshake green means the adapter works
   and the debug *session* does not — likely codesigning/ptrace permissions on macOS or
   Linux. Also a genuine result.
6. **Green on any platform** is the first time a real adapter has been launched for this
   card. Record it here per-platform, with the version banner from the probe report, and
   only then flip the card.

## Verification

```text
cargo fmt --all
cargo test --workspace --all-targets --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- extract-before-modify
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- claim-audit
cargo run -p xtask -- verify-kanban-backlog
cargo run -p xtask -- verify-readiness-consistency
cargo run -p xtask -- dap-adapter-probe --provenance shipped
```

---

## First CI run of the adapter inventory — 2026-08-17

Run: https://github.com/9thLevelSoftware/legion-ide/actions/runs/32042729908

### The one reading that came back

**macOS ships no adapter Legion can find.**

```
dap-adapter-probe: os=macos arch=aarch64 provenance=shipped
  no adapter found under any of: lldb-dap, lldb-vscode, codelldb
  resolvable_adapter_count=0
```

Green with count 0 is a finding, not a pass — as the apparatus's own reading
guide says. `macos-latest` carries Xcode command-line tools and rustc reports
LLVM 22.1.6, but neither puts an `lldb-dap` on `PATH` under any name the
resolver searches. So the working assumption that "CI has adapters, the
developer machine does not" is at best half true, and on macOS it is false.

### Ubuntu and Windows: still unknown, for two different reasons

**Ubuntu failed to build**, before the probe ran:

```
error: failed to run custom build command for `libdbus-sys v0.2.7`
```

`xtask` links the whole workspace, so the job needed the system libraries the
standing gates install and did not have them. Fixed by adding the same
dependency step to both jobs.

This is worth naming precisely, because the job's own comment said it was
"report-only … without turning that into a red run". That was true of the probe
— no `--require`, so finding nothing is recorded rather than failed — and false
of the job, which a build failure reds like any other. The comment now says
which of the two it means. It is a small instance of the pattern this
workstream keeps finding: a claim that is true of the part someone was thinking
about and false of the thing it is attached to.

**Windows never started.** GitHub returned `429 Too Many Requests` for the
`Swatinem/rust-cache` action download, three attempts, then failed the job at
*Set up job*. Nothing was compiled and nothing was probed. That is infrastructure
back-pressure from the volume of CI driven today, not a result.

### Not claimed

**No debug adapter has still ever been launched for this card.** One of three
inventories came back, and it came back empty. `P2.F3.T2` stays `in-progress`.

The macOS reading is one run on one image version
(`macos-latest`, aarch64). Runner images change; this is a dated observation,
not a standing property, which is why the probe exists as a repeatable job
rather than as a sentence in this file.

Whether an adapter can be *provisioned* on macOS is untested — the `dogfood`
job's macOS branch has not yet had a successful run to report on.

---

## The inventory, complete — and the first real adapter launch — 2026-08-17

Run: https://github.com/9thLevelSoftware/legion-ide/actions/runs/32046416145
(after adding the Linux system dependencies the first attempt lacked)

### All three images

```
linux   x86_64   no adapter found under any of: lldb-dap, lldb-vscode, codelldb
                 variant lldb-dap-18 at /usr/bin/lldb-dap-18
                   — not resolvable (resolver searches `lldb-dap` exactly)
                 resolvable_adapter_count=0

windows x86_64   found lldb-dap at C:\Program Files\LLVM\bin\lldb-dap.exe
                   (allowlisted_stem=true) version=(none)
                 resolvable_adapter_count=1

macos   aarch64  no adapter found under any of: lldb-dap, lldb-vscode, codelldb
                 resolvable_adapter_count=0
```

### The versioned-name gap is not hypothetical

It was recorded earlier as a predicted consequence of `apt install lldb`. It is
better than that: **the stock `ubuntu-latest` image already ships a working
`lldb-dap-18` at `/usr/bin/lldb-dap-18`, and Legion cannot see it.** The
resolver searches exact names and the allowlist matches exact stems, so a Linux
user with a perfectly good debugger installed gets "no adapter found".

That moves it from a design smell to a defect with a reproduction on a platform
image anyone can pull. It still belongs in its own task — it is both a
resolution decision (which names to search) and an allowlist decision (which
stems to trust), and widening either casually is how a trust boundary erodes.

### The first launch attempt, and what it says

Windows had an adapter, so the dogfood test ran for real rather than skipping:

```
initialize handshake failed against C:\Program Files\LLVM\bin\lldb-dap.exe:
  DAP session I/O failed: malformed DAP frame: unexpected EOF in headers
```

This is the fourth outcome the apparatus's reading guide anticipated — "adapter
spawned but did not complete `initialize`" — and the test's own module doc
predicted it too ("some runners ship a non-functional adapter").

**But anticipating a failure mode is not diagnosing it.** "Unexpected EOF in
headers" means the child produced no well-formed DAP frame: it may have exited
immediately, written to stderr, or needed arguments Legion does not pass. Three
candidate causes, and the evidence does not yet choose between them:

1. the runner's `lldb-dap.exe` is non-functional as installed;
2. Legion spawns it incorrectly — wrong arguments, wrong working directory, or
   a stdio handle it does not set up;
3. Legion's DAP framing is wrong on Windows.

Cause 2 or 3 would be a product defect and the more important answer. Nothing
here distinguishes them, and the honest reading is "the handshake does not
complete", not "the runner's adapter is broken" — the second is a guess that
happens to be the most comfortable of the three.

### Not claimed

**Still no working debug session, and now a known failure with an unknown
cause.** `P2.F3.T2` stays `in-progress`.

What genuinely advanced: resolution is exercised against a real binary on a real
image, the inventory question is answered for all three platforms, and the
Linux gap is confirmed rather than predicted. What did not: launch, session,
breakpoints, and anything resembling roadmap 1.9's zero-config Rust debugging.

The `dogfood` job is red and should stay red until the handshake question is
answered. It is deliberately not a merge gate — the same posture
`legion-smoke.yml` carries per `T0-D-smoke-promotion-criteria.md`, for the same
reason: a job that gathers evidence about the outside world must not be able to
block unrelated work, and must not be made green to unblock it either.
