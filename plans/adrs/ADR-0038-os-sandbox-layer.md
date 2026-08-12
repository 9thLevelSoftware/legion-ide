# ADR-0038: OS Sandbox Layer

## Status

Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.

This ADR ratifies the Production Master Plan v0.1 §6 recommendation
verbatim (option (a), **bubblewrap (Linux) + Seatbelt profile (macOS) +
restricted token / AppContainer (Windows, weaker, documented) under the
existing capability broker; devcontainer opt-in as the strong tier**)
and records the resulting crate boundary: the kernel-enforced tier
wraps the existing app-layer containment surface (the
`legion-security` capability broker + the
`DelegatedTaskProposalGenerator::validate_containment` containment
path + the `delegated.runtime.allocate` /
`terminal.launch` / `terminal.close` / `terminal.kill` /
`terminal.input` / `terminal.resize` capability reservations) and
enforces the §6 row "kernel-enforced FS-write + network-egress policy
for all Delegate/Workflow shell execution"; `legion-ui` and
`legion-editor` stay projection-only and never own sandbox state;
`legion-app` and `legion-agent` compose the sandbox through the
existing app↔agent↔terminal edge; `legion-cli` and `legion-desktop`
are the entry points that may launch sandboxed Delegate/Workflow
shell execution under the existing capability broker.

## Context

Legion's capability broker is strong app-layer policy (the
`legion-security` crate mediates every privileged action through
capability IDs and principal/consent gating; the existing
`CapabilityBrokerPort` ships as part of the Phase 8 production
dependency rebaseline at `plans/dependency-policy.md` §1 lines
240-250), but the broker by itself cannot prevent a malicious
Delegate/Workflow shell command from escaping the worktree on disk or
opening an outbound network connection the user never approved. The
mid-2026 IDE market has converged on the thesis that **OS-level
sandboxing is a UX feature, not a security detail** (Anthropic reports
84% fewer permission prompts with sandboxing on; Codex ships Landlock
+ seccomp + Seatbelt on Linux and macOS respectively and sandboxes
*by default*; Claude Code and Antigravity market the sandboxing tier
as a first-class user-visible surface; the master plan §4 records the
2026-06-09 verification of the
`anthropic.com/engineering/claude-code-sandboxing` and
`developers.openai.com/codex` sandboxing-internals material). The
master plan §2.2.2 (mid-plan honesty call-out) calls this out
explicitly: app-layer-only enforcement has documented escapes, and
wrapping the trust story around simulated verbs would convince
nobody.

The current Legion state is real and exercised by tests:

- `legion-terminal` (`crates/legion-terminal/src/lib.rs`, 1619
  lines) is the Phase 8 default-deny terminal implementation slice.
  It mediates every PTY operation through the existing capability
  broker: the `terminal.launch` / `terminal.close` / `terminal.kill`
  capability reservations (around lines 1176 / 1197 / 1215) and the
  `terminal.input` / `terminal.resize` reservations in
  `plans/dependency-policy.md` §1 line 251 are the policy surface.
  The two terminal capability-gate unit tests at lines 1255
  (`terminal_fixture_rejects_untrusted_launch`) and 1602
  (`terminal_runtime_rejects_untrusted_policy`) cover the
  default-deny path today; the M0 ratification does not change
  them.
- `legion-platform` (`crates/legion-platform/src/lib.rs`, 2323
  lines) is the Phase 8 production platform layer. Its
  `plans/dependency-policy.md` §1 entry at line 242 authorizes
  `windows` (ConPTY) and either `nix` or `rustix` (Unix PTY,
  process-group, signal handling) as the production PTY
  runtime; the M0 ratification does not change this entry, and the
  WS-12.T2 workstream extends it to authorize the OS-sandbox
  runtime dependencies (bwrap spawn helper, Seatbelt profile
  compiler, AppContainer API, devcontainer CLI) under the same
  production dependency rebaseline that the Phase 8 native PTY
  entry already uses.
- `legion-app` (`crates/legion-app/src/lib.rs`, 23153 lines) is the
  GUI composition crate. The existing delegated-task worktree-
  sandbox substrate lives in `crates/legion-app/src/offline_ai.rs`
  (1443 lines): the `DelegatedTaskProposalGenerator` at line 319
  carries a `sandbox_base: PathBuf` and a
  `validate_containment` at line 410 that canonicalizes the
  sandbox base and rejects any proposal whose `target_path` does
  not canonicalize under it ("path traversal escaped sandbox" at
  line 437). The `delegated.allocate_sandbox` command at line
  14714 of `crates/legion-app/src/lib.rs` is the existing
  app-composition entry point. The three
  `delegated_task_integration` tests in
  `crates/legion-app/tests/delegated_task_integration.rs` at lines
  83 / 156 / 209 assert the
  `execute_delegated_task_waits_for_write_permission_before_sandbox_allocation`
  contract today; the M0 ratification does not change them. The
  WS-12.T2 workstream extends the `validate_containment` path with
  the OS-sandbox containment pre-flight (Seatbelt/bwrap
  profile-compile + profile-apply) without changing the
  capability-broker contract that the existing
  `validate_sandbox_permission` check at
  `offline_ai.rs` line 288 already enforces.
- `legion-agent` (`crates/legion-agent/src/lib.rs`) is the
  Phase 4 metadata-only agent state machine. The
  `plans/dependency-policy.md` §1 entry at line 144 authorizes
  `legion-ai`, `legion-protocol`, and `legion-tracker`. The M0
  ratification does **not** extend `legion-agent`'s allowed
  edges: the agent state machine drives Delegate/Workflow shell
  execution through the `legion-app` composition entry point, and
  the OS-sandbox profile is applied at the `legion-app` /
  `legion-platform` edge, not inside the agent state machine.
  The M0 ratification explicitly forbids `legion-agent` from
  declaring any OS-sandbox runtime dependency (bwrap / Seatbelt
  / AppContainer / devcontainer / Landlock / seccomp-bpf), the
  same way the `ADR-0004` actor model forbids
  `legion-agent` from owning process or filesystem authority.
- `legion-cli` (`crates/legion-cli/`, declared at
  `plans/dependency-policy.md` §1 line 175) is the CLI
  composition crate authorized to depend on `legion-index`,
  `legion-protocol`, and `legion-storage`. The M0 ratification
  does **not** extend `legion-cli`'s allowed edges; the
  WS-12.T2 workstream composes through the existing
  `legion-cli` ↔ `legion-index` / `legion-protocol` /
  `legion-storage` edges plus the `legion-app` composition
  edge that `legion-cli` already has for the existing CLI
  shells. The CLI is one of the two entry points that may launch
  sandboxed Delegate/Workflow shell execution (alongside
  `legion-desktop`); the OS-sandbox profile is applied
  identically from both entry points so the escape-test
  contract is the same.
- `legion-desktop` (`crates/legion-desktop/src/view.rs`) is the
  GUI desktop adapter and the second entry point. The
  `legion-desktop` policy entry at
  `plans/dependency-policy.md` §1 line 77 authorizes `legion-app`,
  `legion-protocol`, and `legion-ui`. The M0 ratification does
  **not** extend `legion-desktop`'s allowed edges: the desktop
  adapter launches sandboxed Delegate/Workflow shell execution
  through the existing `legion-desktop` ↔ `legion-app` edge, and
  the OS-sandbox profile is applied identically from both
  entry points. The `legion-desktop` ↔ `legion-app` ↔
  `legion-platform` chain is the same composition path the
  Phase 2 GUI foundation already uses.
- `legion-ui` (`crates/legion-ui/`) is the projection-only UI
  shell. The `legion-ui` policy entry at
  `plans/dependency-policy.md` §1 lines 54-75 forbids every
  renderer / editor / project / storage / app / agent / terminal
  / security / observability / platform edge and only allows
  `legion-protocol`. The M0 ratification does **not** extend
  `legion-ui`'s allowed edges: the sandbox-status projection
  (the WS-12.T2 acceptance shape) is a new
  `SandboxStatusProjection` family on top of the existing
  `legion-ui` ↔ `legion-protocol` edge, emitted by
  `legion-app` and rendered by `legion-desktop`. `legion-ui`
  never owns sandbox state, never owns the OS-sandbox profile,
  and never owns mutation authority.
- `legion-editor` is the editor substrate. The M0 ratification
  does **not** extend `legion-editor`'s allowed edges
  (forbids any `legion-platform` /
  process / network / terminal / sandbox runtime
  dependency) and the `legion-editor` ↔ `legion-platform`
  edge remains forbidden under the §1
  `legion-editor` MUST NOT rules.

The §2.2 invariants constrain the OS-sandbox layer:

- **App-composed and capability-gated** — the OS-sandbox
  activation is composed through the existing `legion-app`
  composition path, and the `terminal.launch` /
  `terminal.close` / `terminal.kill` / `terminal.input` /
  `terminal.resize` / `delegated.runtime.allocate` capabilities
  are the policy surface. The OS-sandbox activation is a new
  capability-broker decision that is composed **under** the
  existing broker, not in place of it. The WS-12.T2 workstream
  adds the `sandbox.os.activate` /
  `sandbox.os.network.activate` /
  `sandbox.os.fs.activate` capability reservations to
  `plans/dependency-policy.md` §1 line 251 alongside the
  existing Phase 8 production capability names, and the
  broker is the fail-closed policy gate for the kernel-
  enforced tier.
- **Proposal-mediated mutation** — Delegate/Workflow shell
  execution applies edits to buffers or disk only through
  the accepted Phase 2 proposal routes (`ADR-0016`) and the
  AI-plane proposal flow. The OS-sandbox tier enforces the
  filesystem and network boundary; the proposal service
  enforces the approval and the audit boundary. The
  containment pre-flight (the `validate_containment` path
  at `offline_ai.rs` line 410 + the new
  `validate_sandbox_profile` path the WS-12.T2 workstream
  adds) runs before the proposal service emits a proposal
  bundle, so the sandbox profile is part of the proposal
  metadata and visible in the diff surface.
- **Projection-only UI boundary** — `legion-ui` consumes
  sandbox-status projections (`SandboxStatusProjection` /
  `SandboxEscapeAuditProjection` / the future
  `SandboxCapabilityProjection`) and emits
  `CommandDispatchIntent` only. UI never owns sandbox state,
  never owns the OS-sandbox profile, and never owns
  mutation authority. The `legion-ui` policy entry at
  `plans/dependency-policy.md` §1 lines 54-75 already
  forbids every renderer / editor / project / storage /
  app / agent / terminal / security / observability /
  platform edge, and the structural audit enforces it.
  The boundary sketch in this ADR reinforces this rule
  with a future `SANDBOX_BOUNDARY_POLICY_MARKERS` audit
  (no `legion-ui` may declare any
  bubblewrap / Seatbelt / AppContainer / Landlock /
  seccomp / devcontainer runtime dependency), shaped
  like the existing `PARSER_BOUNDARY_POLICY_MARKERS`
  audit in `xtask/src/main.rs` and the
  `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `LSP_BOUNDARY_POLICY_MARKERS` /
  `TERMINAL_BOUNDARY_POLICY_MARKERS` sketches in
  `ADR-0034` / `ADR-0035` / `ADR-0036` / `ADR-0037`.
- **Metadata-first observability** — every sandbox
  activation, profile compile, profile apply, escape
  attempt, and cancellation emits a metadata-only
  observability record: profile hash, scope identity,
  capability used, principal, correlation id, causality
  id, event sequence, sandbox process id, parent process
  id, exit reason. Raw command lines and raw network
  payloads are never emitted; only the
  `sandbox.command.metadata` projection (path, argv
  count, executable hash) is emitted, and the raw argv is
  visible only in the local UI session. The
  observability sinks that reject zero IDs apply to
  sandbox records the same way they apply to terminal
  / AI / tracker / retrieval output. The WS-12.T2
  acceptance shape is a metadata-only escape-audit
  projection (sandbox id, escape class, blocked path,
  blocked destination, blocked syscall, capability
  used) so the existing privacy inspector can render
  the egress surface for the user.
- **Fail-closed policy** — the broker denies an unknown
  capability name; the OS-sandbox profile denies any
  filesystem write outside the worktree; the network
  profile denies any egress outside the allowlist; a
  compile or apply failure is a hard error that
  propagates as `AppCommandError::SandboxActivation`
  and the Delegate/Workflow shell execution is
  cancelled. The `terminal.kill` capability is the
  fail-closed escape hatch: a kill signal to the
  sandbox process group tears down the entire
  process tree, and the cancellation reap path
  (the WS-12.T6 acceptance shape) reaps any
  orphaned sandbox processes on shutdown.

The plan compared three options: (a) the kernel-enforced
tier recorded in §6 (bubblewrap on Linux, Seatbelt profiles
on macOS, restricted token / AppContainer-style constraints
on Windows with honest caveats, devcontainer as the strong
opt-in tier), (b) app-layer only (the status quo), and (c)
microVMs (Firecracker / gVisor-class isolation). Option (a)
matches how the 2026 IDE market converged: Anthropic
shipped Seatbelt + Landlock + seccomp-bpf on Claude Code
and markets the sandboxing tier as a UX feature;
OpenAI's Codex ships Landlock + seccomp + Seatbelt and
sandboxes by default; the master plan §2.2 calls out
that "app-layer-only enforcement has documented escapes"
and that wrapping the trust story around simulated verbs
convinces nobody. Option (b) is the current state — the
`legion-security` capability broker + the
`DelegatedTaskProposalGenerator::validate_containment`
containment path — and is exactly what the §6 row
rejects. Option (c) is the strongest tier but ships
megabytes of sidecars and requires a virt stack that
collides with the 2026 desktop market's expectation
that the IDE install cleanly; the master plan §2.2
records it as a future option that may activate
through devcontainer for users who want the strong
tier.

## Decision

Legion will layer **OS-level sandboxing under the existing
capability broker** for Delegate and Workflow shell
execution: **bubblewrap on Linux**, **Seatbelt profiles
on macOS**, **restricted token / AppContainer-style
constraints on Windows with honest caveats**, and
**devcontainers as the strong opt-in tier** where native
OS support is weaker. The default writable filesystem
scope is the worker worktree. Network egress is
policy-resolved and auditable. Direct writes outside
scope and raw egress attempts must fail closed and
be audited. Sandbox status is visible in the UI.

- **Default OS-sandbox tier (WS-12.T2).** The
  default tier is the kernel-enforced layer
  available on the host OS: bubblewrap (`bwrap`)
  on Linux, Seatbelt profiles compiled from a
  Legion-supplied scheme on macOS, restricted
  token / AppContainer-style constraints on
  Windows. The default tier is the *first* tier
  the Delegate/Workflow shell execution uses,
  and the sandbox status surface in the UI
  reports which tier is active. The default
  tier enforces the §6 row "FS write scope =
  worktree" and "network = policy-resolved
  allowlist via proxy" — on Linux and macOS the
  allowlist is enforced at the kernel layer
  (Seatbelt `network-outbound` rule +
  bubblewrap `--unshare-net` plus a userspace
  proxy that the broker mediates); on Windows
  the AppContainer API provides
  filesystem/network isolation with documented
  weaker guarantees that the UI surfaces
  honestly.
- **Strong opt-in tier — devcontainer
  (WS-12.T2 + WS-20.T1).** Users who want
  stronger isolation than the OS default can
  opt into a devcontainer-backed sandbox: the
  Delegate/Workflow shell execution launches
  inside a developer-supplied
  `.devcontainer/devcontainer.json` that
  declares the OS image, the user, the
  capability set, and the network policy.
  The devcontainer tier is the strong tier on
  hosts where the OS default is weaker
  (Windows, hosts with Seatbelt disabled,
  legacy Linux kernels without Landlock). The
  devcontainer tier is opt-in, never default,
  and the WS-12.T2 acceptance shape verifies
  that the opt-in flow is single-click from
  the sandbox status surface.
- **Capability-broker composition.** The
  OS-sandbox activation is composed through
  the existing `legion-app` composition path
  and the existing `legion-terminal` /
  `legion-platform` PTY boundary. The WS-12.T2
  workstream extends the Phase 8 production
  capability reservations at
  `plans/dependency-policy.md` §1 line 251 with
  `sandbox.os.activate` (the capability that
  gates the kernel-enforced tier),
  `sandbox.os.network.activate` (the capability
  that gates the network allowlist), and
  `sandbox.os.fs.activate` (the capability
  that gates the FS write scope). The broker
  remains the single fail-closed gate: an
  unknown capability name is denied, an
  activation failure is denied, a profile
  compile failure is denied. The broker is
  the OS-sandbox activation audit point: every
  activation, every compile, every apply, and
  every teardown records the principal, the
  capability used, and the correlation id.
- **FS write scope = worktree.** The
  OS-sandbox profile mounts the worker
  worktree (the same
  `sandbox_base: PathBuf` the existing
  `DelegatedTaskProposalGenerator` carries)
  as the only writable mount. The OS-sandbox
  profile denies any write to `$HOME`,
  `/etc`, `/var`, the host filesystem, the
  agent config directory, and any path
  outside the worktree. The `validate_containment`
  app-layer check is the inner ring; the
  OS-sandbox profile is the outer ring; a
  write that escapes the app-layer check
  (e.g. a symlink-following escape) is
  blocked at the kernel layer and audited
  as a metadata-only `SandboxEscapeAuditProjection`.
- **Network egress = policy-resolved
  allowlist.** The OS-sandbox profile
  resolves the network allowlist through the
  existing capability broker: the
  `legion-ai` provider allowlist (per
  `ADR-0006` provider capability flags),
  the `legion-retention` BYOK keyring
  endpoint, the `legion-remote-transport`
  TLS endpoint, the `legion-telemetry`
  hosted-egress endpoint, the
  `legion-collaboration` remote endpoint,
  and any user-approved endpoint from the
  WS-09.T4 privacy inspector. The
  allowlist is enforced at the kernel
  layer (Seatbelt `network-outbound` rule
  + bubblewrap userspace proxy); a raw
  egress attempt is blocked at the kernel
  layer and audited as a metadata-only
  `SandboxEscapeAuditProjection`. The
  hosted-egress endpoints are
  consent-gated per the WS-09.T4
  manifest-to-egress equality test and
  hard-denied in air-gap mode; the OS-sandbox
  profile inherits the air-gap hard-deny
  from the broker.
- **Cancel / reap = fail-closed.** A
  cancellation signal (`terminal.kill` or
  the WS-12.T6 chaos-cancellation path)
  reaps the entire sandbox process group:
  the parent shell, the child processes,
  and any orphaned sandbox subprocesses.
  The OS-sandbox tier provides a process-
  group kill that the broker composes
  with the existing `terminal.kill`
  capability. The reap path is the
  fail-closed escape hatch: a runaway
  process or a stuck tool loop is
  guaranteed to be reaped, and the
  reap is audited with the principal,
  the capability used, and the
  correlation id.
- **Crate boundary.** The OS-sandbox
  layer is split across `legion-platform`,
  `legion-terminal`, `legion-app`,
  `legion-cli`, `legion-agent`, and
  `legion-desktop` along the accepted
  policy entries in
  `plans/dependency-policy.md` §1.
  `legion-platform` owns the OS-sandbox
  runtime: the bubblewrap spawn helper
  (bwrap `Command` wrapper), the
  Seatbelt profile compiler (the
  Legion-supplied SBPL scheme), the
  AppContainer API wrapper
  (Windows-only), and the devcontainer
  CLI shim. The Phase 8 production
  dependency rebaseline at
  `plans/dependency-policy.md` §1 line
  242 is the M0 boundary; the WS-12.T2
  workstream extends it with the
  OS-sandbox runtime dependencies. The
  Legion-supplied Seatbelt scheme is
  a static asset that the sandbox
  profile compiler reads at runtime;
  it is not a runtime dependency, it
  is a build artifact, and the
  `xtask` policy audit does not flag
  static assets. `legion-terminal`
  owns the broker-mediated
  capability integration: the
  `sandbox.os.activate` /
  `sandbox.os.network.activate` /
  `sandbox.os.fs.activate`
  capabilities are added to the
  Phase 8 production capability
  reservation set at
  `plans/dependency-policy.md` §1
  line 251 alongside the existing
  `terminal.launch` /
  `terminal.close` / `terminal.kill` /
  `terminal.input` / `terminal.resize`
  reservations. The
  `legion-terminal` policy entry at
  line 192 already authorizes
  `legion-observability`,
  `legion-platform`, `legion-protocol`,
  and `legion-security`; the M0
  ratification does not extend
  `legion-terminal`'s allowed edges,
  it extends the
  `legion-platform` Phase 8
  production dependency rebaseline.
  `legion-app` owns the
  capability-broker composition: the
  `sandbox.os.activate` capability
  is requested at the
  `delegated.allocate_sandbox`
  composition entry point (the same
  point that already requests
  `delegated.runtime.allocate`),
  and the WS-12.T2 workstream adds
  a `validate_sandbox_profile`
  pre-flight that runs after the
  existing `validate_containment`
  and before the proposal service
  emits a proposal bundle. The
  `legion-app` policy entry at
  line 86 already authorizes the
  full app composition set
  (including `legion-agent`,
  `legion-ai`,
  `legion-ai-providers`,
  `legion-collaboration`,
  `legion-editor`,
  `legion-index`,
  `legion-lsp`,
  `legion-memory`,
  `legion-observability`,
  `legion-platform`,
  `legion-plugin`,
  `legion-project`,
  `legion-protocol`,
  `legion-remote`, plus the
  remaining lines); the M0
  ratification does not extend
  `legion-app`'s allowed edges.
  `legion-cli` owns the CLI
  composition: the `legion-cli
  agent run --sandbox=…` /
  `legion-cli workflow run
  --sandbox=…` /
  `legion-cli evidence check
  --sandbox` flags compose through
  the existing `legion-cli` ↔
  `legion-app` edge. The
  `legion-cli` policy entry at
  line 175 authorizes
  `legion-index`,
  `legion-protocol`, and
  `legion-storage`; the M0
  ratification explicitly forbids
  `legion-cli` from declaring
  any OS-sandbox runtime
  dependency, and the
  `xtask` policy audit enforces
  the boundary by iterating the
  same `package_dependencies`
  map that drives the
  renderer-boundary and
  parser-boundary checks.
  `legion-agent` owns the agent
  state machine: the agent
  drives Delegate/Workflow
  shell execution through the
  `legion-app` composition
  entry point, and the
  OS-sandbox profile is
  applied at the
  `legion-app` /
  `legion-platform` edge,
  not inside the agent state
  machine. The `legion-agent`
  policy entry at line 144
  authorizes `legion-ai`,
  `legion-protocol`, and
  `legion-tracker`; the M0
  ratification explicitly
  forbids `legion-agent`
  from declaring any
  OS-sandbox runtime
  dependency, and the
  `xtask` policy audit
  enforces the boundary.
  `legion-desktop` owns the
  desktop adapter composition:
  the desktop adapter
  launches sandboxed
  Delegate/Workflow shell
  execution through the
  existing
  `legion-desktop` ↔
  `legion-app` edge, and
  the OS-sandbox profile
  is applied identically
  from both entry points
  so the escape-test
  contract is the same.
  The `legion-desktop`
  policy entry at line 77
  authorizes `legion-app`,
  `legion-protocol`, and
  `legion-ui`; the M0
  ratification does not
  extend `legion-desktop`'s
  allowed edges. `legion-ui`
  and `legion-editor` may
  **not** declare any
  OS-sandbox runtime
  dependency. The
  `legion-ui` policy entry
  at `plans/dependency-policy.md`
  §1 lines 54-75 already
  forbids every renderer /
  editor / project /
  storage / app / agent /
  terminal / security /
  observability / platform
  edge, and the structural
  audit enforces it. The
  boundary sketch in this
  ADR reinforces this rule
  with a future
  `SANDBOX_BOUNDARY_POLICY_MARKERS`
  audit (no `legion-ui`
  may declare any
  bubblewrap / Seatbelt /
  AppContainer / Landlock /
  seccomp / devcontainer
  runtime dependency),
  shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS`
  audit in
  `xtask/src/main.rs` and
  the
  `SEARCH_BOUNDARY_POLICY_MARKERS`
  /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS`
  /
  `LSP_BOUNDARY_POLICY_MARKERS`
  /
  `TERMINAL_BOUNDARY_POLICY_MARKERS`
  sketches in `ADR-0034` /
  `ADR-0035` / `ADR-0036` /
  `ADR-0037`. The M0
  ratification does not
  require the
  sandbox-boundary audit
  to land today; the
  audit is a phase-gate
  improvement that
  becomes useful the
  moment a workspace
  package actually
  declares one of the
  forbidden OS-sandbox
  runtime crates. Today,
  no package declares
  any of them, so the
  audit is a
  forward-compatibility
  gate, not a regression
  guard.
- **Compatibility with `ADR-0004`
  (actor model + async
  runtime).** The
  OS-sandbox layer is
  composed through the
  existing actor model
  and the existing
  `tokio` async runtime
  the `legion-platform`
  Phase 8 production
  rebaseline already
  uses. The bubblewrap
  spawn helper, the
  Seatbelt profile
  compiler, the
  AppContainer API
  wrapper, and the
  devcontainer CLI
  shim are all
  composed inside the
  existing
  `legion-platform`
  process-group
  abstraction and
  use the existing
  `tokio` async
  runtime. The
  `terminal.kill`
  capability reaps
  the sandbox process
  group; the reap
  path is the
  fail-closed
  escape hatch
  described above.
- **Compatibility with
  `ADR-0006` (AI
  provider
  abstraction).**
  The network
  allowlist is
  resolved through
  the existing
  `legion-ai` /
  `legion-ai-providers`
  provider
  capability flags
  (per `ADR-0006`),
  so the same
  provider
  capabilities
  that gate chat
  and completion
  gate the
  embedding egress
  and the
  vector-tier
  egress. The
  WS-12.T2
  acceptance
  shape
  verifies
  that the
  network
  allowlist
  in the
  OS-sandbox
  profile is
  derived
  from the
  same
  provider
  capabilities
  that the
  AI plane
  uses.
- **Compatibility
  with the
  air-gap
  story.**
  The
  OS-sandbox
  profile
  inherits
  the
  air-gap
  hard-deny
  from the
  broker:
  in air-gap
  mode the
  `sandbox.os.network.activate`
  capability
  is denied,
  the
  network
  allowlist
  is empty,
  and the
  OS-sandbox
  profile
  denies
  every
  outbound
  socket
  (Seatbelt
  `network-outbound`
  deny-all
  rule,
  bubblewrap
  `--unshare-net`,
  AppContainer
  no-network
  capability).
  The
  devcontainer
  tier
  inherits
  the same
  air-gap
  hard-deny.
  The
  offline
  `legion-app`
  path
  remains
  the
  air-gap
  default.
- **Honest
  Windows
  caveats.**
  The
  Windows
  AppContainer
  API
  provides
  filesystem
  and
  network
  isolation
  with
  documented
  weaker
  guarantees
  than
  bubblewrap
  or
  Seatbelt:
  it
  is
  a
  user-mode
  isolation
  primitive,
  not
  a
  kernel-mode
  one,
  and
  it
  does
  not
  prevent
  all
  classes
  of
  escape
  that
  the
  Linux
  and
  macOS
  tiers
  prevent.
  The
  UI
  surfaces
  this
  honestly
  (the
  `SandboxStatusProjection`
  includes
  the
  tier
  and
  the
  documented
  caveat),
  the
  threat-model
  doc
  (the
  WS-20.T1
  acceptance
  shape)
  records
  the
  caveat
  publicly,
  and
  the
  devcontainer
  tier
  is
  the
  one-click
  escape
  hatch
  for
  Windows
  users
  who
  want
  kernel-grade
  isolation.
  The
  M0
  ratification
  forbids
  the
  marketing
  surface
  from
  promising
  uniform
  guarantees
  across
  the
  three
  OSes
  and
  commits
  the
  UI
  and
  the
  threat-model
  doc
  to
  honest
  tiering.

## Consequences

- **Positive:**
  the
  M0
  ratification
  ratifies
  a
  kernel-enforced
  trust
  tier
  that
  wraps
  the
  existing
  app-layer
  capability
  broker
  and
  the
  existing
  `validate_containment`
  containment
  path.
  The
  OS-sandbox
  layer
  is
  the
  outer
  ring
  of
  the
  trust
  story;
  the
  capability
  broker
  is
  the
  inner
  ring;
  the
  proposal
  service
  is
  the
  approval
  and
  audit
  ring;
  the
  OS-sandbox
  status
  projection
  is
  the
  user-visible
  ring.
  The
  four
  rings
  compose
  into
  the
  trust
  story
  the
  master
  plan
  §4
  records
  as
  the
  2026
  market
  expectation.
- **Positive:**
  the
  WS-12
  workstreams
  (T1
  tool
  registry
  + execution
  loop,
  T2
  OS
  sandbox
  layer,
  T3
  plan
  mode,
  T4
  Delegate
  golden
  path,
  T5
  context
  management,
  T6
  failure
  & recovery,
  T7
  subagent
  fan-out)
  have
  a
  real
  starting
  point
  in
  `crates/legion-app/src/lib.rs`
  (the
  `delegated.allocate_sandbox`
  composition
  entry
  point
  at
  line
  14714),
  `crates/legion-app/src/offline_ai.rs`
  (the
  `DelegatedTaskProposalGenerator::validate_containment`
  path
  at
  line
  410),
  `crates/legion-terminal/src/lib.rs`
  (the
  `terminal.launch`
  /
  `terminal.close`
  /
  `terminal.kill`
  capability
  reservations
  around
  lines
  1176-1215),
  and
  `crates/legion-platform/src/lib.rs`
  (the
  Phase
  8
  production
  PTY
  rebaseline
  at
  `plans/dependency-policy.md`
  §1
  line
  242).
  The
  existing
  `legion-app`
  composition
  edges
  to
  `legion-agent`,
  `legion-ai`,
  `legion-ai-providers`,
  `legion-platform`,
  `legion-security`,
  `legion-protocol`,
  and
  the
  rest
  of
  the
  GUI
  Phase
  4
  set
  are
  already
  policy-allowed;
  the
  OS-sandbox
  composition
  is
  a
  new
  capability-broker
  decision
  that
  is
  composed
  under
  the
  existing
  broker,
  not
  a
  new
  crate
  edge.
- **Positive:**
  air-gap
  mode
  is
  preserved.
  The
  OS-sandbox
  profile
  inherits
  the
  air-gap
  hard-deny
  from
  the
  broker;
  the
  devcontainer
  tier
  inherits
  the
  same
  hard-deny;
  the
  offline
  `legion-app`
  path
  remains
  the
  air-gap
  default.
  The
  hosted
  provider
  activation
  flow
  from
  WS-09.T4
  remains
  the
  consent-gated
  path
  for
  embedding
  and
  completion
  egress,
  and
  the
  OS-sandbox
  network
  allowlist
  is
  derived
  from
  the
  same
  provider
  capabilities
  that
  the
  AI
  plane
  uses.
- **Negative:**
  Windows
  guarantees
  are
  weaker
  and
  must
  not
  be
  marketed
  as
  equivalent.
  The
  UI
  surfaces
  this
  honestly
  (the
  `SandboxStatusProjection`
  includes
  the
  tier
  and
  the
  documented
  caveat),
  the
  threat-model
  doc
  (the
  WS-20.T1
  acceptance
  shape)
  records
  the
  caveat
  publicly,
  and
  the
  devcontainer
  tier
  is
  the
  one-click
  escape
  hatch
  for
  Windows
  users
  who
  want
  kernel-grade
  isolation.
  The
  M0
  ratification
  forbids
  the
  marketing
  surface
  from
  promising
  uniform
  guarantees
  across
  the
  three
  OSes
  and
  commits
  the
  UI
  and
  the
  threat-model
  doc
  to
  honest
  tiering.
- **Negative:**
  declaring
  the
  OS-sandbox
  runtime
  dependencies
  is
  a
  WS-12.T2
  runtime
  activation,
  not
  an
  M0
  prerequisite.
  The
  M0
  ratification
  ratifies
  the
  boundary
  and
  the
  tier
  choice;
  the
  WS-12.T2
  ("OS
  sandbox
  layer
  (ADR-0038)")
  task
  is
  the
  workstream
  that
  declares
  the
  dependencies,
  extends
  the
  `legion-platform`
  Phase
  8
  production
  rebaseline
  at
  `plans/dependency-policy.md`
  §1
  line
  242
  to
  authorize
  the
  bubblewrap
  spawn
  helper
  /
  Seatbelt
  profile
  compiler
  /
  AppContainer
  API
  wrapper
  /
  devcontainer
  CLI
  shim,
  extends
  the
  Phase
  8
  production
  capability
  reservation
  set
  at
  line
  251
  with
  the
  `sandbox.os.activate`
  /
  `sandbox.os.network.activate`
  /
  `sandbox.os.fs.activate`
  capabilities,
  adds
  the
  sandbox-boundary
  audit,
  and
  ships
  the
  product
  code
  that
  the
  §6
  row
  describes.
- **Mitigation:**
  the
  `legion-platform`
  Phase
  8
  production
  rebaseline
  at
  `plans/dependency-policy.md`
  §1
  line
  242
  is
  the
  only
  edge
  that
  needs
  the
  new
  OS-sandbox
  runtime
  dependencies,
  the
  Phase
  8
  capability
  reservation
  set
  at
  line
  251
  is
  the
  only
  edge
  that
  needs
  the
  new
  `sandbox.os.*`
  capabilities,
  and
  the
  future
  `SANDBOX_BOUNDARY_POLICY_MARKERS`
  audit
  is
  the
  structural
  guard
  that
  prevents
  any
  other
  crate
  from
  declaring
  them.
  The
  structural
  dependency
  audit
  that
  already
  runs
  as
  part
  of
  `cargo
  run
  -p
  xtask
  --
  check-deps`
  is
  the
  M0
  test
  surface;
  the
  WS-12.T2
  acceptance
  criteria
  are
  the
  escape-attempt
  /
  air-gap
  /
  cancel-reap
  test
  surface.

## Verification

- `cargo
  run
  -p
  xtask
  --
  check-deps`
  (dependency
  direction
  +
  structural
  audit,
  with
  the
  `legion-platform`,
  `legion-terminal`,
  `legion-app`,
  `legion-cli`,
  `legion-agent`,
  `legion-desktop`,
  `legion-ui`,
  and
  `legion-editor`
  policy
  entries
  verified
  against
  `plans/dependency-policy.md`
  §1
  and
  the
  sandbox-boundary
  sketch
  above)
- `cargo
  run
  -p
  xtask
  --
  docs-hygiene`
  (broken
  relative
  Markdown
  links
  and
  the
  unallowlisted
  stale
  Legion-rename
  marker)
- `cargo
  run
  -p
  xtask
  --
  no-egui-textedit`
  (companion
  gate,
  unchanged
  from
  `ADR-0032`;
  the
  sandbox-status
  panel
  renders
  projected
  sandbox
  results,
  not
  an
  `egui::TextEdit`)
- `cargo
  fmt
  --all
  --check`
- `cargo
  test
  -p
  legion-terminal
  --tests`
  (the
  Phase
  8
  default-deny
  terminal
  implementation
  slice
  and
  the
  `terminal.launch`
  /
  `terminal.close`
  /
  `terminal.kill`
  capability-gate
  unit
  tests
  at
  lines
  1255
  /
  1602
  that
  the
  OS-sandbox
  broker
  composition
  extends)
- `cargo
  test
  -p
  legion-platform
  --tests`
  (the
  Phase
  8
  production
  platform
  layer
  and
  the
  PTY
  rebaseline
  that
  the
  OS-sandbox
  spawn
  helper
  composes
  with)
- `cargo
  test
  -p
  legion-app
  --tests`
  (the
  `delegated.allocate_sandbox`
  composition
  entry
  point
  at
  `crates/legion-app/src/lib.rs`
  line
  14714
  and
  the
  `DelegatedTaskProposalGenerator::validate_containment`
  path
  at
  `crates/legion-app/src/offline_ai.rs`
  line
  410,
  including
  the
  three
  `delegated_task_integration`
  tests
  in
  `crates/legion-app/tests/delegated_task_integration.rs`
  that
  the
  OS-sandbox
  broker
  composition
  extends)
- `cargo
  test
  -p
  legion-cli
  --tests`
  (the
  CLI
  composition
  entry
  point
  and
  the
  `legion-cli
  evidence
  check`
  /
  `agent
  run`
  /
  `workflow
  run`
  flag
  surface
  that
  the
  WS-12.T2
  workstream
  extends
  with
  the
  `--sandbox`
  flag)
- `cargo
  test
  -p
  legion-security
  --tests`
  (the
  capability-broker
  contract
  and
  the
  `CapabilityBrokerPort`
  that
  the
  OS-sandbox
  broker
  composition
  extends;
  the
  `legion-security`
  policy
  entry
  at
  line
  20
  authorizes
  `legion-protocol`
  only,
  and
  the
  OS-sandbox
  broker
  composition
  is
  a
  new
  capability
  decision
  composed
  through
  the
  existing
  `CapabilityBrokerPort`,
  not
  a
  new
  crate
  edge)
- `cargo
  test
  -p
  legion-ai
  --tests`
  and
  `cargo
  test
  -p
  legion-ai-providers
  --tests`
  (the
  provider
  capability
  flags
  per
  `ADR-0006`
  that
  the
  OS-sandbox
  network
  allowlist
  is
  derived
  from,
  unchanged
  from
  `ADR-0037`)
- `cargo
  test
  -p
  legion-agent
  --tests`
  (the
  agent
  state
  machine
  that
  drives
  Delegate/Workflow
  shell
  execution
  through
  the
  `legion-app`
  composition
  entry
  point)
- `cargo
  test
  -p
  legion-desktop
  --tests`
  (the
  desktop
  adapter
  composition
  that
  launches
  sandboxed
  Delegate/Workflow
  shell
  execution
  through
  the
  existing
  `legion-desktop`
  ↔
  `legion-app`
  edge)
- WS-12
  evidence
  under
  `plans/evidence/production/m2/`
  once
  the
  tool
  registry
  +
  execution
  loop
  (WS-12.T1)
  and
  the
  OS
  sandbox
  layer
  (WS-12.T2)
  land
  with
  dependency-policy
  updates
  and
  contract
  tests;
  WS-12.T3
  plan-mode
  evidence
  under
  the
  same
  directory
  once
  the
  plan
  mode
  surface
  lands;
  WS-12.T4
  Delegate
  golden-path
  evidence
  under
  the
  same
  directory
  once
  the
  end-to-end
  fixture
  lands;
  WS-12.T5
  context-management
  evidence
  under
  the
  same
  directory
  once
  the
  token-budget
  +
  compaction
  +
  cache-aware
  prompt
  layout
  lands;
  WS-12.T6
  failure
  &
  recovery
  evidence
  under
  the
  same
  directory
  once
  the
  stuck-detection
  +
  cancellation
  +
  reap
  surface
  lands;
  WS-12.T7
  subagent
  fan-out
  evidence
  under
  the
  same
  directory
  once
  the
  parent/child
  audit
  chain
  lands.
  M0
  ratification
  does
  not
  require
  any
  of
  these
  WS-12
  evidence
  packages
  today;
  the
  M0
  evidence
  package
  for
  this
  ratification
  is
  `plans/evidence/production/M0/ADR-0038-ratification.md`.
- WS-12.T2
  escape-attempt
  test
  suite
  (write
  outside
  scope,
  raw
  egress,
  symlink-following
  escape,
  process-group
  reap)
  is
  the
  M1/M2
  acceptance
  shape
  for
  the
  OS-sandbox
  layer.
  The
  suite
  is
  the
  contract
  test
  surface
  that
  the
  `legion-platform`
  WS-12.T2
  workstream
  ships,
  and
  the
  suite
  is
  the
  M3
  exit
  criterion
  (the
  master
  plan
  §8
  records
  "sandbox
  escape
  suite
  green"
  as
  the
  M3
  exit).
  The
  M0
  ratification
  ratifies
  the
  suite
  shape
  as
  a
  future
  gate,
  not
  as
  an
  M0
  prerequisite.
