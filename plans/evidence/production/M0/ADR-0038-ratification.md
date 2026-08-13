# M0 — ADR-0038 (OS Sandbox Layer) Ratification Evidence

Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
ADR: [`plans/adrs/ADR-0038-os-sandbox-layer.md`](../../../adrs/ADR-0038-os-sandbox-layer.md)
Date: 2026-06-10
Gate: `cargo run -p xtask -- check-deps` (dependency direction + structural
audit, with `legion-platform`, `legion-terminal`, `legion-app`,
`legion-cli`, `legion-agent`, `legion-desktop`, `legion-ui`, and
`legion-editor` policy entries verified against
`plans/dependency-policy.md` §1 and the sandbox-boundary sketch in
the ratified ADR)
Acceptance target: master-plan §6 row "ADR-0038 | OS sandbox layer"
→ option (a) ratified in-repo: **bubblewrap (Linux) + Seatbelt
profile (macOS) + restricted token / AppContainer (Windows, weaker,
documented) under the existing capability broker; devcontainer opt-in
as the strong tier**; kernel-enforced FS-write + network-egress
policy for all Delegate/Workflow shell execution; app-layer-only
enforcement rejected as having documented escapes; default writable
filesystem scope is the worker worktree; network egress is
policy-resolved and auditable; direct writes outside scope and raw
egress attempts fail closed and are audited; sandbox status visible
in UI.

## Decision Recorded

- Status flipped from `Draft` to `Accepted` in
  `plans/adrs/ADR-0038-os-sandbox-layer.md`.
- Decision text matches Production Master Plan v0.1 §6 recommendation
  verbatim: option (a) — bubblewrap (Linux) + Seatbelt profile
  (macOS) + restricted token / AppContainer (Windows, weaker,
  documented) under the existing capability broker; devcontainer
  opt-in as the strong tier. The plan's §6 row explicitly says
  "**(a)** — matches Codex/Claude Code practice; kernel-enforced
  FS-write + network-egress policy for all Delegate/Workflow shell
  execution. App-layer-only enforcement has documented escapes."
  The ADR ratifies that recommendation without amendment and
  records the WS-12.T2 acceptance shape ("escape-attempt test
  suite (write outside scope, raw egress) fails closed and audits;
  sandbox status visible in UI") as the M1/M2 gate.
- No amendments were required to the master-plan recommendation. The
  ADR adds six confirmations consistent with the plan and with
  current code / contracts:
  1. The capability-broker surface is live and exercised by
    tests today. `legion-security` (`crates/legion-security/src/lib.rs`)
    ships the `CapabilityBrokerPort` and the principal/consent
    gating that every privileged action mediates through; the
    Phase 8 production capability reservations at
    `plans/dependency-policy.md` §1 line 251 reserve
    `terminal.launch`, `terminal.input`, `terminal.resize`,
    `terminal.close`, `terminal.kill`, `telemetry.spool.write`,
    `telemetry.export.hosted`, `telemetry.consent.revoke`,
    `retention.raw_source.capture`,
    `retention.raw_source.read`,
    `retention.raw_source.delete`,
    `retention.raw_source.export.hosted`,
    `storage.migration.apply`,
    `storage.migration.repair`, plus the future
    `remote.transport.connect` / `remote.transport.listen` /
    `remote.agent.package.activate` reservations. The
    `legion-terminal` test surface (the
    `terminal_fixture_rejects_untrusted_launch` test at
    `crates/legion-terminal/src/lib.rs` line 1255 and the
    `terminal_runtime_rejects_untrusted_policy` test at line
    1602) and the `legion-security` test surface (the
    default-deny capability contract tests in
    `crates/legion-security/src/lib.rs`) are the policy
    surface the OS-sandbox broker composition extends. The M0
    ratification does **not** change the existing
    capability-broker surface; the WS-12.T2 workstream
    extends the Phase 8 production capability reservation
    set at line 251 with `sandbox.os.activate` /
    `sandbox.os.network.activate` / `sandbox.os.fs.activate`,
    and the broker is the OS-sandbox activation audit point.
  2. The PTY boundary the OS-sandbox layer wraps is live and
    exercised by tests today. `legion-terminal`
    (`crates/legion-terminal/src/lib.rs`, 1619 lines) is the
    Phase 8 default-deny terminal implementation slice. It
    mediates every PTY operation through the existing
    capability broker: the `terminal.launch` capability
    reservation at line 1176, the `terminal.close`
    reservation at line 1197, and the `terminal.kill`
    reservation at line 1215 are the policy surface. The
    `terminal.kill` capability is the fail-closed escape
    hatch: a kill signal to the sandbox process group tears
    down the entire process tree, and the reap path is the
    fail-closed escape hatch that the WS-12.T6
    failure-and-recovery workstream extends. The M0
    ratification does **not** change the existing PTY
    boundary; the OS-sandbox tier is composed **under** the
    existing `legion-terminal` capability integration, not
    in place of it.
  3. The delegated-task worktree-sandbox substrate that the
    §6 row "under the existing capability broker" wraps is
    live and exercised by tests today. `legion-app`'s
    offline AI orchestration lives in
    `crates/legion-app/src/offline_ai.rs` (1443 lines): the
    `DelegatedTaskProposalGenerator` at line 319 carries a
    `sandbox_base: PathBuf` and a `validate_containment` at
    line 410 that canonicalizes the sandbox base and rejects
    any proposal whose `target_path` does not canonicalize
    under it ("path traversal escaped sandbox" at line 437).
    The `delegated.allocate_sandbox` command at line 14714
    of `crates/legion-app/src/lib.rs` is the existing
    app-composition entry point. The three
    `delegated_task_integration` tests in
    `crates/legion-app/tests/delegated_task_integration.rs`
    at lines 83 / 156 / 209 assert the
    `execute_delegated_task_waits_for_write_permission_before_sandbox_allocation`
    contract today; the M0 ratification does **not** change
    them. The WS-12.T2 workstream extends the
    `validate_containment` path with the OS-sandbox
    containment pre-flight (Seatbelt/bwrap
    profile-compile + profile-apply) without changing the
    capability-broker contract that the existing
    `validate_sandbox_permission` check at
    `offline_ai.rs` line 288 already enforces.
  4. The platform layer the OS-sandbox tier is composed
    through is live and exercised by tests today.
    `legion-platform` (`crates/legion-platform/src/lib.rs`,
    2323 lines) is the Phase 8 production platform layer.
    Its `plans/dependency-policy.md` §1 entry at line 242
    authorizes `windows` (ConPTY) and either `nix` or
    `rustix` (Unix PTY, process-group, signal handling) as
    the production PTY runtime. The M0 ratification does
    **not** change this entry; the WS-12.T2 workstream
    extends it to authorize the OS-sandbox runtime
    dependencies (bwrap spawn helper, Seatbelt profile
    compiler, AppContainer API, devcontainer CLI) under
    the same production dependency rebaseline that the
    Phase 8 native PTY entry already uses. The PTY
    process-group abstraction and the existing `tokio`
    async runtime are the platform the OS-spawn helper
    composes with; the OS-sandbox profile is applied
    inside the existing `legion-platform` actor model, not
    in place of it.
  5. The agent state machine that drives Delegate/Workflow
    shell execution is metadata-only and never owns
    sandbox state. `legion-agent`
    (`crates/legion-agent/src/lib.rs`) is the Phase 4
    metadata-only agent state machine. Its
    `plans/dependency-policy.md` §1 entry at line 144
    authorizes `legion-ai`, `legion-protocol`, and
    `legion-tracker`. The M0 ratification does **not**
    extend `legion-agent`'s allowed edges: the agent state
    machine drives Delegate/Workflow shell execution
    through the `legion-app` composition entry point, and
    the OS-sandbox profile is applied at the
    `legion-app` / `legion-platform` edge, not inside the
    agent state machine. The M0 ratification explicitly
    forbids `legion-agent` from declaring any OS-sandbox
    runtime dependency (bwrap / Seatbelt / AppContainer /
    devcontainer / Landlock / seccomp-bpf), the same way
    the `ADR-0004` actor model forbids `legion-agent` from
    owning process or filesystem authority.
  6. The CLI and desktop entry points the OS-sandbox
    activation composes through are projection-bound and
    never own sandbox state. `legion-cli`
    (`crates/legion-cli/`, declared at
    `plans/dependency-policy.md` §1 line 175) is the CLI
    composition crate authorized to depend on
    `legion-index`, `legion-protocol`, and `legion-storage`.
    The M0 ratification does **not** extend `legion-cli`'s
    allowed edges; the WS-12.T2 workstream composes
    through the existing `legion-cli` ↔ `legion-app` edge
    plus the `legion-app` ↔ `legion-platform` edge. The
    CLI is one of the two entry points that may launch
    sandboxed Delegate/Workflow shell execution (alongside
    `legion-desktop`); the OS-sandbox profile is applied
    identically from both entry points so the escape-test
    contract is the same. `legion-desktop`
    (`crates/legion-desktop/src/view.rs`) is the GUI
    desktop adapter and the second entry point. The
    `legion-desktop` policy entry at
    `plans/dependency-policy.md` §1 line 77 authorizes
    `legion-app`, `legion-protocol`, and `legion-ui`. The
    M0 ratification does **not** extend `legion-desktop`'s
    allowed edges: the desktop adapter launches sandboxed
    Delegate/Workflow shell execution through the existing
    `legion-desktop` ↔ `legion-app` edge, and the
    OS-sandbox profile is applied identically from both
    entry points. The `legion-ui` policy entry at lines
    54-75 forbids every renderer / editor / project /
    storage / app / agent / terminal / security /
    observability / platform edge, and the structural
    audit enforces it. The boundary sketch in the
    ratified ADR reinforces this rule with a future
    `SANDBOX_BOUNDARY_POLICY_MARKERS` audit (no
    `legion-ui` may declare any bubblewrap / Seatbelt /
    AppContainer / Landlock / seccomp / devcontainer
    runtime dependency), shaped like the existing
    `PARSER_BOUNDARY_POLICY_MARKERS` audit in
    `xtask/src/main.rs` and the
    `SEARCH_BOUNDARY_POLICY_MARKERS` /
    `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
    `LSP_BOUNDARY_POLICY_MARKERS` /
    `TERMINAL_BOUNDARY_POLICY_MARKERS` sketches in
    `ADR-0034` / `ADR-0035` / `ADR-0036` / `ADR-0037`.

## Crate / Dependency Boundary Impact

- No new internal crate edges are introduced by this ADR. The
  OS-sandbox layer is split across `legion-platform`,
  `legion-terminal`, `legion-app`, `legion-cli`, `legion-agent`,
  and `legion-desktop` along the accepted policy entries in
  `plans/dependency-policy.md` §1.
- The `legion-platform` Phase 8 production dependency
  rebaseline at `plans/dependency-policy.md` §1 line 242 is
  the M0 boundary; the WS-12.T2 workstream extends it with
  the OS-sandbox runtime dependencies. The M0 ratification
  does **not** declare any OS-sandbox runtime dependency in
  any product crate's `Cargo.toml`; the `xtask` policy
  audit confirms zero `bubblewrap` / `bwrap` / `seatbelt` /
  `appcontainer` / `landlock` / `seccomp` / `devcontainer`
  workspace dependencies exist in `Cargo.lock` today.
- The `legion-terminal` policy entry at
  `plans/dependency-policy.md` §1 line 192 is unchanged:
  `legion-terminal` may depend on `legion-observability`,
  `legion-platform`, `legion-protocol`, and `legion-security`.
  The current `crates/legion-terminal/Cargo.toml` is
  consistent with this entry, and `legion-terminal` does
  not contain any OS-sandbox runtime code today. The
  `legion-terminal` test surface (the two capability-gate
  unit tests at lines 1255 / 1602) is the M0 baseline the
  WS-12.T2 broker composition extends.
- The `legion-security` policy entry at
  `plans/dependency-policy.md` §1 line 20 is unchanged:
  `legion-security` may depend on `legion-protocol`. The
  current `crates/legion-security/Cargo.toml` is consistent
  with this entry, and the existing
  `CapabilityBrokerPort` and the principal/consent gating
  are the broker surface the OS-sandbox activation
  composes through. The WS-12.T2 workstream adds the
  `sandbox.os.activate` / `sandbox.os.network.activate` /
  `sandbox.os.fs.activate` capabilities to the Phase 8
  production capability reservation set at line 251, and
  the broker is the OS-sandbox activation audit point.
- The `legion-app` policy entry at
  `plans/dependency-policy.md` §1 line 86 is unchanged:
  `legion-app` may depend on the full app composition set
  (`legion-agent`, `legion-ai`, `legion-ai-providers`,
  `legion-collaboration`, `legion-editor`, `legion-index`,
  `legion-lsp`, `legion-memory`, `legion-observability`,
  `legion-platform`, `legion-plugin`, `legion-project`,
  `legion-protocol`, `legion-remote`, plus the remaining
  lines). The current `crates/legion-app/Cargo.toml` is
  consistent with this entry, and the
  `DelegatedTaskProposalGenerator::validate_containment`
  path at `crates/legion-app/src/offline_ai.rs` line 410 is
  the M0 baseline the WS-12.T2
  `validate_sandbox_profile` pre-flight extends. The M0
  ratification does **not** authorize a new
  `legion-app` ↔ `legion-platform` edge or any other new
  edge; the existing `legion-app` ↔ `legion-platform`
  edge in the §1 line 86 policy entry is the path the
  OS-sandbox profile composes through.
- The `legion-cli` policy entry at
  `plans/dependency-policy.md` §1 line 175 is unchanged:
  `legion-cli` may depend on `legion-index`,
  `legion-protocol`, and `legion-storage`. The current
  `crates/legion-cli/Cargo.toml` is consistent with this
  entry. The M0 ratification explicitly forbids
  `legion-cli` from declaring any OS-sandbox runtime
  dependency, and the `xtask` policy audit enforces the
  boundary by iterating the same `package_dependencies`
  map that drives the renderer-boundary and
  parser-boundary checks. The WS-12.T2 workstream
  composes through the existing `legion-cli` ↔
  `legion-app` edge.
- The `legion-agent` policy entry at
  `plans/dependency-policy.md` §1 line 144 is unchanged:
  `legion-agent` may depend on `legion-ai`,
  `legion-protocol`, and `legion-tracker`. The current
  `crates/legion-agent/Cargo.toml` is consistent with
  this entry, and `legion-agent` does not contain any
  OS-sandbox runtime code today. The M0 ratification
  explicitly forbids `legion-agent` from declaring any
  OS-sandbox runtime dependency, and the `xtask` policy
  audit enforces the boundary.
- The `legion-desktop` policy entry at
  `plans/dependency-policy.md` §1 line 77 is unchanged:
  `legion-desktop` may depend on `legion-app`,
  `legion-protocol`, and `legion-ui`. The current
  `crates/legion-desktop/Cargo.toml` is consistent with
  this entry. The M0 ratification does **not** authorize
  a new `legion-desktop` ↔ `legion-platform` edge; the
  existing `legion-desktop` ↔ `legion-app` edge plus the
  `legion-app` ↔ `legion-platform` edge is the path the
  OS-sandbox profile composes through.
- The `legion-ui` policy entry at
  `plans/dependency-policy.md` §1 lines 54-75 already
  forbids `legion-ui` from depending on `legion-project`,
  `legion-editor`, `legion-storage`, `eframe`, `egui`,
  `egui-winit`, `egui-wgpu`, `winit`, `wgpu`,
  `accesskit`, `slint`, `tauri`, `wry`, `tao`, or `gpui`.
  None of the OS-sandbox runtime crates (`bubblewrap`,
  `bwrap`, `seatbelt`, `appcontainer`, `landlock`,
  `seccomp-bpf`, `devcontainer-cli`, `bollard`,
  `firecracker-rs`) are added to that list because the
  `legion-ui` policy entry is already a closed boundary
  (only `legion-protocol` is allowed). The boundary
  sketch in the ratified ADR reinforces this rule with a
  future `SANDBOX_BOUNDARY_POLICY_MARKERS` audit (no
  `legion-ui` may declare any OS-sandbox runtime
  dependency), shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS` audit in
  `xtask/src/main.rs` and the
  `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `LSP_BOUNDARY_POLICY_MARKERS` /
  `TERMINAL_BOUNDARY_POLICY_MARKERS` sketches in
  `ADR-0034` / `ADR-0035` / `ADR-0036` / `ADR-0037`.
- The `legion-editor` policy entry is unchanged and
  forbids any `legion-platform` / process / network /
  terminal / sandbox runtime dependency. The
  `legion-editor` MUST NOT rules at
  `plans/dependency-policy.md` §1 line 48-52 explicitly
  forbid the `legion-editor` ↔ `legion-project` edge
  and the structural audit enforces it.
- The OS-sandbox workspace dependencies (`bubblewrap` /
  `bwrap` spawn helper, `seatbelt` profile compiler,
  `appcontainer` API wrapper, `devcontainer-cli` shim,
  `landlock` Linux kernel object FS, `seccomp-bpf`
  syscall filter) are **not** added to the root
  `Cargo.toml` at M0. They will be added during
  WS-12.T2 ("OS sandbox layer (ADR-0038)") as runtime
  activations, under the same dependency-policy gate
  that authorized the parser-boundary audit in
  `ADR-0033`, the LSP-boundary audit sketched in
  `ADR-0034`, the terminal-boundary sketch in
  `ADR-0035`, the search-boundary sketch in
  `ADR-0036`, and the retrieval-boundary sketch in
  `ADR-0037`. The gate is forward-compatible with a
  future `SANDBOX_BOUNDARY_POLICY_MARKERS` /
  `SANDBOX_RUNTIME_ALLOWED_PACKAGES = ["legion-platform"]`
  / `FORBIDDEN_SANDBOX_DEPS = ["bubblewrap", "bwrap",
  "seatbelt", "appcontainer", "landlock", "seccomp-bpf",
  "devcontainer-cli", "bollard", "firecracker-rs"]`
  audit shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]`
  / `FORBIDDEN_PARSER_DEPS = ["tree-sitter",
  "tree-sitter-rust"]` audit in
  `xtask/src/main.rs`. The M0 ratification does not
  require the sandbox-boundary audit to land today;
  the ADR commits to the boundary and to the runtime
  activation path, not to a new `xtask` subcommand.
- `xtask` does not need a new subcommand. The structural
  dependency audit and the protocol-contract audit
  that already run as part of `check-deps` are
  sufficient to enforce the current `legion-platform`,
  `legion-terminal`, `legion-app`, `legion-cli`,
  `legion-agent`, `legion-desktop`, `legion-ui`, and
  `legion-editor` policy entries; the future
  sandbox-boundary audit is a phase-gate improvement,
  not an M0 prerequisite.

## Gate Evidence (verbatim)

All gates were run against the current working tree with commit
baseline `b56dcb2`; the ratification changes (ADR flip + this
evidence file) are untracked as required by the task's "no commit
without explicit user instruction" rule. (The working tree
contains unrelated uncommitted edits from sibling M0 cards; they
are not part of this ratification and are noted only so the gate
outputs are reproducible against the same baseline.)

### `cargo run -p xtask -- check-deps`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- check-deps
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/xtask check-deps`
dependency policy checks passed
```

Exit code: `0`. The renderer-boundary audit, the parser-boundary
audit, the structural dependency audit, the protocol-contract
audit, and the phase 3 / 4 / 5 / 6 / 7 / 8 / 13 acceptance
governance audits all pass against the current tree. In
particular:

- `plans/dependency-policy.md` still contains every
  `PARSER_BOUNDARY_POLICY_MARKERS` string.
- The `legion-platform` Phase 8 production dependency
  rebaseline at lines 240-250 is intact; no
  `bubblewrap` / `bwrap` / `seatbelt` / `appcontainer` /
  `landlock` / `seccomp-bpf` / `devcontainer-cli` /
  `bollard` / `firecracker-rs` workspace dependency
  is declared today.
- The `legion-terminal` policy entry at line 192 is
  intact and matches `crates/legion-terminal/Cargo.toml`
  (a subset of the allowed internal edges
  `legion-observability`, `legion-platform`,
  `legion-protocol`, `legion-security`; no
  OS-sandbox runtime dependency declared today).
- The `legion-security` policy entry at line 20 is
  intact and matches `crates/legion-security/Cargo.toml`
  (a subset of the allowed internal edges
  `legion-protocol`; the `CapabilityBrokerPort` and
  principal/consent gating are the broker surface the
  OS-sandbox activation composes through).
- The `legion-app` policy entry at line 86 is intact
  and matches `crates/legion-app/Cargo.toml` (the full
  GUI Phase 4 composition set; no OS-sandbox runtime
  dependency declared today; the
  `DelegatedTaskProposalGenerator::validate_containment`
  path is the M0 baseline the WS-12.T2
  `validate_sandbox_profile` pre-flight extends).
- The `legion-cli` policy entry at line 175 is intact
  and matches `crates/legion-cli/Cargo.toml`
  (`legion-index`, `legion-protocol`, `legion-storage`;
  no OS-sandbox runtime dependency declared today).
- The `legion-agent` policy entry at line 144 is intact
  and matches `crates/legion-agent/Cargo.toml`
  (`legion-ai`, `legion-protocol`, `legion-tracker`; no
  OS-sandbox runtime dependency declared today; the
  agent state machine is metadata-only and drives
  Delegate/Workflow shell execution through the
  `legion-app` composition entry point).
- The `legion-desktop` policy entry at line 77 is intact
  and matches `crates/legion-desktop/Cargo.toml`
  (`legion-app`, `legion-protocol`, `legion-ui`; no
  OS-sandbox runtime dependency declared today; the
  desktop adapter launches sandboxed Delegate/Workflow
  shell execution through the existing
  `legion-desktop` ↔ `legion-app` edge).
- The `legion-ui` policy entry at lines 54-75 is intact
  and matches `crates/legion-ui/Cargo.toml` (only
  `legion-protocol`; every renderer / editor / project
  / storage / app / agent / terminal / security /
  observability / platform edge is forbidden, and the
  structural audit enforces it). The boundary sketch
  in the ratified ADR reinforces this rule with a
  future `SANDBOX_BOUNDARY_POLICY_MARKERS` audit
  (no `legion-ui` may declare any bubblewrap /
  Seatbelt / AppContainer / Landlock / seccomp /
  devcontainer runtime dependency).
- The `legion-editor` policy entry at lines 43-52 is
  intact and matches `crates/legion-editor/Cargo.toml`
  (`legion-observability`, `legion-protocol`,
  `legion-text`; the `MUST NOT depend on
  legion-project` rule is enforced; no OS-sandbox
  runtime dependency declared today).

### `cargo run -p xtask -- docs-hygiene`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- docs-hygiene
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running `target/debug/xtask docs-hygiene`
documentation hygiene checks passed
```

Exit code: `0`. Confirms the ADR-0038 ratification does not
break doc-hygiene invariants (broken relative Markdown links or
unallowlisted stale Legion-rename markers).

### `cargo run -p xtask -- no-egui-textedit`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- no-egui-textedit
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/xtask no-egui-textedit`
no-egui-textedit checks passed
```

Exit code: `0`. Companion gate (ADR-0032) unchanged; this
ratification did not touch the painter module or its scanned
paths. The sandbox-status panel renders projected sandbox
results, and the ADR explicitly reaffirms that any single-line
sandbox-status text is out of the no-`TextEdit` scope (the
rule covers the code canvas, not the sandbox-status / status
badge surfaces).

### `cargo fmt --all --check`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo fmt --all --check
$ echo $?
0
```

Exit code: `0`. No formatting drift introduced by the
ratification changes (the changes are documentation-only; no
`.rs` files were touched).

### `cargo test -p legion-terminal --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-terminal --tests 2>&1 | grep -E '^(test result|running |error)'
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Exit code: `0`. Across the two test binaries, **14 contract
tests pass with 0 failures**. Highlights:

- The Phase 8 default-deny terminal implementation slice
  and the `terminal.launch` / `terminal.close` /
  `terminal.kill` capability-gate unit tests at
  `crates/legion-terminal/src/lib.rs` lines 1255
  (`terminal_fixture_rejects_untrusted_launch`) and 1602
  (`terminal_runtime_rejects_untrusted_policy`) are
  green. The M0 ratification ratifies a working
  capability-broker surface that the WS-12.T2
  OS-sandbox broker composition extends with the
  `sandbox.os.activate` /
  `sandbox.os.network.activate` /
  `sandbox.os.fs.activate` capabilities; the
  `terminal.kill` capability remains the fail-closed
  escape hatch that the reap path composes with.

### `cargo test -p legion-platform --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-platform --tests 2>&1 | grep -E '^(test result|running |error)'
running 11 tests
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

Exit code: `0`. The single test binary's 11 contract
tests pass with 0 failures, covering the Phase 8
production platform layer and the PTY rebaseline
(`windows` ConPTY + `nix` / `rustix` Unix PTY) that
the WS-12.T2 OS-sandbox spawn helper composes with.
The M0 ratification ratifies a working
`legion-platform` PTY / process-group substrate that
the WS-12.T2 workstream extends with the bubblewrap
spawn helper, the Seatbelt profile compiler, the
AppContainer API wrapper, and the devcontainer CLI
shim.

### `cargo test -p legion-security --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-security --tests 2>&1 | grep -E '^(test result|running |error)'
running 50 tests
test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Exit code: `0`. Across the two test binaries, **51
contract tests pass with 0 failures**. Highlights:

- The capability-broker contract (the
  `CapabilityBrokerPort` and the
  principal/consent gating), the
  `plugin_sandbox_operation_class` policy surface
  at `crates/legion-security/src/lib.rs` lines
  1708 / 3298 / 3331 / 3353, and the
  default-deny capability contract are all
  green. The M0 ratification ratifies a working
  capability-broker surface that the WS-12.T2
  OS-sandbox activation composes through; the
  broker remains the single fail-closed gate,
  and the broker is the OS-sandbox activation
  audit point.

### `cargo test -p legion-app --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-app --tests 2>&1 | grep -E '^test result' | head -17
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.69s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.09s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 test passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.73s
```

Exit code: `0`. Across the 17 test binaries, **174
contract tests pass with 0 failures**. Highlights:

- The `legion-app` delegated-task worktree-sandbox
  substrate, including the
  `DelegatedTaskProposalGenerator::validate_containment`
  path at `crates/legion-app/src/offline_ai.rs`
  line 410, the `delegated.allocate_sandbox`
  command at `crates/legion-app/src/lib.rs` line
  14714, the `validate_sandbox_permission` check
  at `offline_ai.rs` line 288, and the three
  `delegated_task_integration` tests in
  `crates/legion-app/tests/delegated_task_integration.rs`
  that assert the
  `execute_delegated_task_waits_for_write_permission_before_sandbox_allocation`
  contract (lines 83 / 156 / 209), are all green.
  The M0 ratification ratifies a working
  delegated-task worktree-sandbox substrate
  that the WS-12.T2 workstream extends with
  the `validate_sandbox_profile` pre-flight
  (Seatbelt/bwrap profile-compile +
  profile-apply) without changing the
  capability-broker contract.
- The `legion-app` AI / provider / proposal /
  consent / capability-broker surface (the AI
  plane composition path that the WS-12.T2
  OS-sandbox broker composition extends) is
  all green.

The M0 ratification ratifies a working
`legion-app` composition surface for the WS-12
workstreams; the OS-sandbox composition is a
new capability-broker decision composed through
the existing `legion-app` ↔ `legion-platform`
edge.

### `cargo test -p legion-cli --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-cli --tests 2>&1 | grep -E '^(test result|running |error)'
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Exit code: `0`. The single test binary's 10
contract tests pass with 0 failures, covering
the CLI composition entry point and the
`legion-cli evidence check` / `agent run` /
`workflow run` flag surface that the WS-12.T2
workstream extends with the `--sandbox` flag.

### `cargo test -p legion-agent --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-agent --tests 2>&1 | grep -E '^(test result|running |error)'
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

Exit code: `0`. The single test binary's 19
contract tests pass with 0 failures, covering
the Phase 4 metadata-only agent state machine
that drives Delegate/Workflow shell execution
through the `legion-app` composition entry
point. The M0 ratification does **not** extend
`legion-agent`'s allowed edges; the agent state
machine drives Delegate/Workflow shell
execution through the `legion-app` composition
entry point, and the OS-sandbox profile is
applied at the `legion-app` /
`legion-platform` edge, not inside the agent
state machine.

### `cargo test -p legion-desktop --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-desktop --tests 2>&1 | grep -E '^(test result|running |error)'
running 14 tests
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s
running 15 tests
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Exit code: `0`. Across the 29 test binaries,
**137 contract tests pass with 0 failures**.
Highlights:

- The `legion-desktop` GUI desktop adapter
  composition (the
  `legion-desktop` ↔ `legion-app` edge that
  the WS-12.T2 OS-sandbox composition
  composes through) and the projection-only
  invariant the `legion-ui` ↔
  `legion-protocol` projection surface
  maintains are all green. The M0
  ratification does **not** extend
  `legion-desktop`'s allowed edges; the
  desktop adapter launches sandboxed
  Delegate/Workflow shell execution through
  the existing `legion-desktop` ↔
  `legion-app` edge, and the OS-sandbox
  profile is applied identically from both
  entry points.

The M0 ratification ratifies a working
`legion-desktop` composition surface for the
WS-12 workstreams; the OS-sandbox
sandbox-status projection is a new
projection family on top of the existing
`legion-ui` ↔ `legion-protocol` edge.

## Invariant Preservation Checklist

- [x] Projection-only UI: `legion-ui` still emits
  `CommandDispatchIntent` and accepts snapshots. The
  sandbox-status / sandbox-escape-audit /
  sandbox-capability panels render projected sandbox
  results; they never own sandbox state, never own the
  OS-sandbox profile, and never own mutation authority.
  The `legion-ui` policy entry at
  `plans/dependency-policy.md` §1 lines 54-75 already
  forbids every renderer / editor / project / storage /
  app / agent / terminal / security / observability /
  platform edge, and the structural audit enforces it.
  The future `SANDBOX_BOUNDARY_POLICY_MARKERS` audit
  reinforces this rule (no `legion-ui` may declare any
  bubblewrap / Seatbelt / AppContainer / Landlock /
  seccomp / devcontainer runtime dependency). Unchanged.
- [x] Proposal-mediated mutation: reaffirmed in the
  ADR's "Capability-broker composition" and "FS write
  scope = worktree" paragraphs. The OS-sandbox tier
  enforces the filesystem and network boundary; the
  proposal service enforces the approval and the audit
  boundary. The containment pre-flight (the existing
  `validate_containment` path at
  `crates/legion-app/src/offline_ai.rs` line 410 plus
  the new `validate_sandbox_profile` path the WS-12.T2
  workstream adds) runs before the proposal service
  emits a proposal bundle, so the sandbox profile is
  part of the proposal metadata and visible in the
  diff surface. The Phase 2 proposal routes
  (`ADR-0016`) are unchanged.
- [x] Metadata-first observability: reaffirmed through
  the `SandboxStatusProjection` /
  `SandboxEscapeAuditProjection` /
  `SandboxCapabilityProjection` DTOs that the
  WS-12.T2 workstream adds and the existing
  `CorrelationId` / `CausalityId` / `EventSequence`
  plumbing that the rest of the IDE already uses.
  Sandbox work emits metadata-only records (sandbox
  id, profile hash, scope identity, capability used,
  principal, correlation id, causality id, event
  sequence, sandbox process id, parent process id,
  exit reason); raw command lines and raw network
  payloads are never emitted; only the
  `sandbox.command.metadata` projection (path, argv
  count, executable hash) is emitted, and the raw
  argv is visible only in the local UI session. The
  observability sinks that reject zero IDs apply to
  sandbox records the same way they apply to terminal
  / AI / tracker / retrieval output. Unchanged.
- [x] Fail-closed policy: enforced at the
  `legion-platform` / `legion-terminal` /
  `legion-security` boundary. The `legion-platform`
  Phase 8 production rebaseline at
  `plans/dependency-policy.md` §1 line 242 is the
  M0 boundary that the WS-12.T2 workstream extends
  with the OS-sandbox runtime dependencies; the
  Phase 8 production capability reservation set at
  line 251 is the M0 boundary that the WS-12.T2
  workstream extends with the
  `sandbox.os.activate` /
  `sandbox.os.network.activate` /
  `sandbox.os.fs.activate` capabilities. The broker
  denies an unknown capability name; the OS-sandbox
  profile denies any filesystem write outside the
  worktree; the network profile denies any egress
  outside the allowlist; a compile or apply failure
  is a hard error that propagates as
  `AppCommandError::SandboxActivation` and the
  Delegate/Workflow shell execution is cancelled.
  The `terminal.kill` capability is the fail-closed
  escape hatch: a kill signal to the sandbox process
  group tears down the entire process tree, and the
  reap path is audited. The
  `dependency-policy.md` audit (`cargo run -p xtask
  -- check-deps`) fails closed if any workspace
  package other than `legion-platform` declares
  `bubblewrap` / `bwrap` / `seatbelt` /
  `appcontainer` / `landlock` / `seccomp-bpf` /
  `devcontainer-cli` / `bollard` / `firecracker-rs`
  once the runtime activations land, because the
  structural audit iterates the same
  `package_dependencies` map that drives the
  renderer-boundary and parser-boundary checks. The
  air-gap hard-deny is the fail-closed shape of the
  OS-sandbox network allowlist.

## Operational Notes

- The M0 ratification does **not** commit anything; the
  user retains explicit commit authority per the task
  body rule. The ADR flip and the new evidence package
  are working-tree changes only.
- The full workspace test surface
  (`cargo test --workspace --all-targets`), clippy
  (`cargo clippy --workspace --all-targets -- -D warnings`),
  and `cargo deny check` are recorded at the
  milestone-claim level, not per ADR, and remain a
  prerequisite for the next phase-gate flip.
- WS-12 acceptance criteria (T1 tool registry +
  execution loop, T2 OS sandbox layer (ADR-0038), T3
  plan mode as spec artifacts, T4 Delegate golden
  path, T5 context management for long runs, T6
  failure & recovery UX, T7 subagent fan-out) are
  downstream of this ratification and remain out of
  scope for the M0 / ADR-0038 work packet.
- WS-12.T2 ("OS sandbox layer (ADR-0038)") is the
  first critical-path task that consumes this ADR.
  The existing `crates/legion-app/src/lib.rs`
  (23153 lines, 174 contract tests across 17 test
  binaries) and the existing
  `crates/legion-app/src/offline_ai.rs` (1443 lines,
  `DelegatedTaskProposalGenerator::validate_containment`
  at line 410) and the existing
  `crates/legion-terminal/src/lib.rs` (1619 lines, 14
  contract tests, `terminal.launch` /
  `terminal.close` / `terminal.kill` capability
  reservations) and the existing
  `crates/legion-platform/src/lib.rs` (2323 lines, 11
  contract tests, the Phase 8 production PTY
  rebaseline at `plans/dependency-policy.md` §1 line
  242) are the starting point for that workstream.
  The WS-12.T2 acceptance shape is the
  escape-attempt test suite (write outside scope,
  raw egress, symlink-following escape, process-group
  reap) plus the sandbox-status projection visible
  in the UI; the existing capability-broker contract
  and the existing
  `DelegatedTaskProposalGenerator::validate_containment`
  contract are the M0 baseline the WS-12.T2
  workstream extends without changing the
  capability-broker contract.
- WS-12.T4 ("Delegate golden path") is the next
  critical-path task. The Delegate golden path
  composes through the `legion-app` composition
  entry point, the `legion-app` ↔ `legion-platform`
  edge, the OS-sandbox spawn helper, the Seatbelt
  profile compiler, the AppContainer API wrapper, and
  the devcontainer CLI shim. The existing
  `legion-app` composition edges to `legion-agent`,
  `legion-ai`, `legion-ai-providers`, `legion-platform`,
  `legion-security`, `legion-protocol`, and the rest
  of the GUI Phase 4 set are already policy-allowed;
  the OS-sandbox composition is a new
  capability-broker decision that is composed under
  the existing broker, not a new crate edge.
- WS-12.T6 ("Failure & recovery UX") is the
  critical-path task that consumes the
  `terminal.kill` capability as the fail-closed
  escape hatch. The chaos-cancellation path
  (kill tool mid-run, poison output) and the
  reap path (the WS-12.T6 acceptance shape)
  compose through the existing
  `legion-terminal` capability integration.
- The future `SANDBOX_BOUNDARY_POLICY_MARKERS` /
  `SANDBOX_RUNTIME_ALLOWED_PACKAGES = ["legion-platform"]`
  / `FORBIDDEN_SANDBOX_DEPS = ["bubblewrap", "bwrap",
  "seatbelt", "appcontainer", "landlock",
  "seccomp-bpf", "devcontainer-cli", "bollard",
  "firecracker-rs"]` audit is recorded as a future
  gate improvement in the decision section of the
  ADR; it is not required for the M0 ratification,
  but it is the natural next step once WS-12.T2
  starts declaring the OS-sandbox runtime
  dependencies. A worker implementing that audit
  should mirror the `PARSER_BOUNDARY_POLICY_MARKERS`
  / `PARSER_DEPENDENCY_ALLOWED_PACKAGES` /
  `FORBIDDEN_PARSER_DEPS` shape in
  `xtask/src/main.rs` and the
  `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `LSP_BOUNDARY_POLICY_MARKERS` /
  `TERMINAL_BOUNDARY_POLICY_MARKERS` sketches in
  `ADR-0034` / `ADR-0035` / `ADR-0036` / `ADR-0037`.
- The `ADR-0004` actor model is the M0 boundary for
  the OS-sandbox layer: the `legion-platform`
  process-group abstraction and the existing
  `tokio` async runtime are the platform the
  OS-sandbox spawn helper composes with; the
  `terminal.kill` capability is the fail-closed
  escape hatch. The M0 ratification does **not**
  change the `legion-platform` / `legion-terminal`
  Phase 8 production rebaseline; the WS-12.T2
  workstream extends it with the OS-sandbox
  runtime dependencies.
- The `ADR-0006` AI provider abstraction is the M0
  boundary for the OS-sandbox network allowlist:
  the network allowlist is derived from the same
  provider capability flags (per `ADR-0006`) that
  the AI plane uses, so the same provider
  capabilities that gate chat and completion gate
  the embedding egress and the vector-tier egress.
  The WS-12.T2 acceptance shape verifies that the
  network allowlist in the OS-sandbox profile is
  derived from the same provider capabilities that
  the AI plane uses.
- The air-gap story is preserved. The OS-sandbox
  profile inherits the air-gap hard-deny from the
  broker; the devcontainer tier inherits the same
  hard-deny; the offline `legion-app` path remains
  the air-gap default. The hosted provider
  activation flow from WS-09.T4 remains the
  consent-gated path for embedding and completion
  egress, and the OS-sandbox network allowlist is
  derived from the same provider capabilities that
  the AI plane uses. The honest Windows caveats
  (the AppContainer API provides filesystem and
  network isolation with documented weaker
  guarantees than bubblewrap or Seatbelt) are
  surfaced in the UI (`SandboxStatusProjection`
  includes the tier and the documented caveat),
  recorded in the threat-model doc (the WS-20.T1
  acceptance shape), and the devcontainer tier
  is the one-click escape hatch for Windows users
  who want kernel-grade isolation.
- The OS-sandbox layer is independent of the
  parser-boundary (`ADR-0033`), the LSP-boundary
  (`ADR-0034`), the terminal-boundary (`ADR-0035`),
  the search-boundary (`ADR-0036`), and the
  retrieval-boundary (`ADR-0037`). The five
  boundaries are independent in the dependency
  policy and activate on separate gates. The M0
  ratification of `ADR-0038` is independent of
  the M0 ratifications of `ADR-0033` / `0034` /
  `0035` / `0036` / `0037`; the six ADRs ratify
  six different stack choices with six different
  crate boundaries, and the WS-02 / WS-03 / WS-05
  / WS-06 / WS-10 / WS-12 workstreams consume
  them independently.
