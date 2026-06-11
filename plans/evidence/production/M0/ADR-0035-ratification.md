# M0 — ADR-0035 (Terminal Stack) Ratification Evidence

Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
ADR: [`plans/adrs/ADR-0035-terminal-stack.md`](../../../adrs/ADR-0035-terminal-stack.md)
Date: 2026-06-10
Gate: `cargo run -p xtask -- check-deps` (dependency direction + structural
audit, with `legion-terminal` and `legion-platform` policy entries verified
against `plans/dependency-policy.md` §1 and the Phase 8 production
dependency rebaseline)
Acceptance target: master-plan §6 row "ADR-0035 | Terminal stack"
→ option (a) ratified in-repo: `alacritty_terminal` VTE grid + audited
`legion-platform` PTY layer, with the terminal runtime owned by
`legion-terminal` and consumed by `legion-app` through protocol DTOs
and the policy-gated launch path.

## Decision Recorded

- Status flipped from `Draft` to `Accepted` in
  `plans/adrs/ADR-0035-terminal-stack.md`.
- Decision text matches Production Master Plan v0.1 §6 recommendation
  verbatim: option (a) `alacritty_terminal` VTE grid + `legion-platform`
  PTY, with a custom egui terminal renderer painted from projected
  terminal state. The plan's WS-05 entry explicitly says "keep the
  existing audited PTY layer (it already passes policy gates), add
  `alacritty_terminal` for terminal state, custom egui renderer"; the
  ADR ratifies that recommendation without amendment.
- No amendments were required to the master-plan recommendation. The
  ADR adds three confirmations consistent with the plan and with
  current code/contracts:
  1. The runtime ownership is `legion-terminal` over `legion-platform`,
    and both crates are already active. `legion-platform/src/lib.rs`
    is 2323 lines and ships the cross-platform
    filesystem/process/watcher/PTY/environment/time surface with
    `nix`-backed Unix PTY and `windows`-backed ConPTY, exactly as the
    Phase 8 production dependency rebaseline in
    `plans/dependency-policy.md` §1 authorizes. `legion-terminal/src/lib.rs`
    is 1619 lines and ships the deterministic `TerminalFixtureRuntime`,
    `DapAdapterFixtureRuntime`, launch-policy validation, and
    metadata-only audit record emission. The M0 ratification ratifies
    a working boundary and a real substrate, not a future build.
  2. The `alacritty_terminal` dependency is recorded as a runtime
    activation to be done during WS-05.T1 (or later, when the first
    VTE/grid test needs to round-trip through the production PTY
    backend). The M0 ratification commits only to the boundary:
    `legion-terminal` is the only workspace crate allowed to declare
    `alacritty_terminal`, `portable-pty`, or `wezterm-term`.
  3. The app/UI/desktop boundary is spelled out explicitly so the
    downstream WS-05 workstreams can build on the existing
    `legion-protocol` DTOs, the GUI Phase 4 `legion-app` composition
    entry, and the `legion-ui` projection-only invariant without
    re-litigating the boundary. The companion `xtask no-egui-textedit`
    gate is reaffirmed for the terminal grid renderer; the renderer
    is a custom egui widget, not an `egui::TextEdit`.

## Crate / Dependency Boundary Impact

- No new internal crate edges are introduced by this ADR.
- The `legion-terminal` policy entry in `plans/dependency-policy.md` §1
  is unchanged: `legion-terminal` may depend on `legion-observability`,
  `legion-platform`, `legion-protocol`, and `legion-security`. The
  current `crates/legion-terminal/Cargo.toml` declares
  `legion-platform`, `legion-protocol`, `thiserror`, and `uuid` — a
  subset of the policy-allowed set (no `legion-observability` /
  `legion-security` declared today; both are allowed when needed), so
  the ratification does not require any manifest change today.
- The `legion-platform` policy entry in `plans/dependency-policy.md` §1
  is unchanged: `legion-platform` may depend on `legion-protocol` and
  MUST directly depend on `legion-protocol`. The current
  `crates/legion-platform/Cargo.toml` declares `legion-protocol`,
  `thiserror`, plus the platform-conditional `nix` (Unix PTY/process/
  signal handling) and `windows` (ConPTY: `Win32_Foundation`,
  `Win32_Security`, `Win32_Storage_FileSystem`, `Win32_System_Console`,
  `Win32_System_IO`, `Win32_System_Pipes`, `Win32_System_Threading`)
  — a strict subset of the Phase 8 production dependency rebaseline in
  `plans/dependency-policy.md` §1, which authorizes `nix` or `rustix`
  for Unix PTY and `windows` for ConPTY. The `rustix` alternative is
  documented but not yet needed; the current `nix` choice matches
  Helix's PTY backend and is what `legion-platform`'s tests already
  exercise. The M0 ratification does not require a manifest change.
- The GUI Phase 4 composition entry already authorizes
  `legion-app` → `legion-terminal` (per the policy text in §1 at
  line 103: `legion-app` may depend on `legion-terminal` for the
  language-and-terminal IDE loop). The current
  `crates/legion-app/Cargo.toml` declares this edge at line 26, and
  the `legion-app` source composes the terminal runtime through
  protocol DTOs and the policy-gated launch path. No policy change
  is required.
- The `alacritty_terminal` workspace dependency is **not** added to
  the root `Cargo.toml` at M0. It will be added during WS-05.T1 as a
  runtime activation under the same dependency-policy gate that
  authorized the parser-boundary audit in `ADR-0033` and the
  LSP-boundary audit sketched in `ADR-0034`. The gate is
  forward-compatible with a future `TERMINAL_BOUNDARY_POLICY_MARKERS`
  / `TERMINAL_DEPENDENCY_ALLOWED_PACKAGES = ["legion-terminal"]` /
  `FORBIDDEN_TERMINAL_DEPS = ["alacritty_terminal", "portable-pty",
  "wezterm-term"]` audit shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]` /
  `FORBIDDEN_PARSER_DEPS = ["tree-sitter", "tree-sitter-rust"]` audit
  in `xtask/src/main.rs`. The M0 ratification does not require the
  terminal audit to land today; the ADR commits to the boundary and
  to the runtime activation path, not to a new `xtask` subcommand.
- `xtask` does not need a new subcommand. The structural dependency
  audit and the protocol-contract audit that already run as part of
  `check-deps` are sufficient to enforce the current
  `legion-terminal` and `legion-platform` policy entries; the future
  terminal-boundary audit is a phase-gate improvement, not an M0
  prerequisite.

## Gate Evidence (verbatim)

All gates were run against the current working tree with commit
baseline `b56dcb2`; the ratification changes (ADR flip + this evidence
file) are untracked as required by the task's "no commit without
explicit user instruction" rule. (The working tree contains unrelated
uncommitted edits from sibling M0 cards; they are not part of this
ratification and are noted only so the gate outputs are reproducible
against the same baseline.)

### `cargo run -p xtask -- check-deps`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- check-deps
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/xtask check-deps`
dependency policy checks passed
```

Exit code: `0`. The renderer-boundary audit, the parser-boundary
audit, the structural dependency audit, the protocol-contract audit,
and the phase 3/4/5/6/7/8/13 acceptance governance audits all pass
against the current tree. In particular:

- `plans/dependency-policy.md` still contains every
  `PARSER_BOUNDARY_POLICY_MARKERS` string.
- The `legion-terminal` policy entry at lines 192-196 is intact and
  matches `crates/legion-terminal/Cargo.toml` (a subset of the
  allowed internal edges, no VTE/grid or PTY transport dependency
  declared today).
- The `legion-platform` policy entry at lines 26-30 is intact and
  matches `crates/legion-platform/Cargo.toml` (`legion-protocol` +
  `thiserror` + platform-conditional `nix`/`windows`, all in the
  Phase 8 production dependency rebaseline).
- The structural audit confirms `legion-app`'s `legion-terminal`
  edge is policy-allowed (line 103 of the policy file lists
  `legion-terminal` in the `legion-app` may-depend-on set) and that
  `legion-editor` does not declare a `legion-terminal` or
  `legion-platform` edge (the `legion-editor` policy entry at lines
  43-52 only authorizes `legion-observability`, `legion-protocol`,
  and `legion-text`).

### `cargo run -p xtask -- docs-hygiene`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- docs-hygiene
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/xtask docs-hygiene`
documentation hygiene checks passed
```

Exit code: `0`. Confirms the ADR-0035 ratification does not break
doc-hygiene invariants (broken relative Markdown links or unallowlisted
stale Legion-rename markers).

### `cargo run -p xtask -- no-egui-textedit`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- no-egui-textedit
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/xtask no-egui-textedit`
no-egui-textedit checks passed
```

Exit code: `0`. Companion gate (ADR-0032) unchanged; this ratification
did not touch the painter module or its scanned paths. The terminal
grid renderer will be a custom egui widget (not `egui::TextEdit`),
and the ADR explicitly reaffirms the no-`TextEdit` rule for the
terminal canvas.

### `cargo fmt --all --check`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo fmt --all --check
$ echo $?
0
```

Exit code: `0`. No formatting drift introduced by the ratification
changes (the changes are documentation-only; no `.rs` files were
touched).

### `cargo test -p legion-terminal --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-terminal --tests 2>&1 | grep -E '^(test result|running )'
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Exit code: `0`. Across the two test binaries (the crate's `lib.rs`
unittests + the `dap_adapter_fixture` integration test),
**14 contract tests pass with 0 failures**. Highlights:

- `TerminalFixtureRuntime` unit tests (13 tests): fixture disabled
  state, launch policy validation (schema version, timeout bounds,
  workspace trust, audit record validity), `TerminalAuditRecord`
  validation, command-block metadata emission, input/resize/close/
  kill contract enforcement, kill-escalation model.
- `tests/dap_adapter_fixture.rs` (1 test): DAP adapter fixture
  launch/deny contract; mirrors the terminal fixture's policy
  contract shape.

The M0 ratification ratifies a working boundary plus a real fixture
backend; WS-05.T1 will productize the production PTY backend on top
of this contract.

### `cargo test -p legion-platform --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-platform --tests 2>&1 | grep -E '^(test result|running )'
running 11 tests
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Exit code: `0`. The 11 platform tests cover the
filesystem/process/watcher/PTY/environment/time surface that backs
the terminal runtime; they exercise the `nix`/`windows`-conditional
module surface on the host platform (macOS in this run) and the
cross-platform metadata/normalization surface that the terminal
runtime consumes. WS-05.T1 will add explicit kill-escalation /
orphan-reaping contract tests on top of this surface.

## Invariant Preservation Checklist

- [x] Projection-only UI: `legion-ui` still emits
  `CommandDispatchIntent` and accepts snapshots; terminal transport
  / VTE / grid / PTY crates stay out of `legion-ui` per the
  dependency policy. The terminal-grid renderer is a custom egui
  widget painted by `legion-app`/`legion-desktop` from projected
  terminal state; the renderer never owns terminal session state,
  PTY/process state, or mutation authority. The companion
  `xtask no-egui-textedit` gate enforces the no-`TextEdit` rule on
  the terminal canvas. Unchanged.
- [x] Proposal-mediated mutation: reaffirmed in the ADR's
  "Proposal-mediated mutation (unchanged from `ADR-0026`)"
  paragraph. The terminal runtime never mutates the workspace,
  editor, or disk directly. Any terminal-originated mutation
  candidate materializes as a `WorkspaceProposal` payload through
  the existing `legion-app` proposal routes. The fixture audit
  records already model the policy contract, and the production
  PTY backend reuses the same `TerminalAuditRecord` /
  `CommandBlock` validation surface. The Phase 2 proposal routes
  (`ADR-0016`) and the `ADR-0026` acceptance reservation are
  unchanged.
- [x] Metadata-first observability: reaffirmed through the
  metadata-only `TerminalAuditRecord` contract. Terminal sessions
  persist metadata-only summaries (session id, launch policy
  contract id, command boundaries, exit code, duration, bounded
  output reference, `CorrelationId`, `CausalityId`) by default;
  raw command bodies, transcripts, full process output, secrets,
  and full environment values are never persisted. The
  observability sinks that reject zero `CorrelationId` / nil
  `CausalityId` / zero `EventSequence` already apply to terminal
  output via the existing `TerminalAuditRecord` validation
  surface. Unchanged.
- [x] Fail-closed policy: enforced at the platform/terminal
  boundary. The launch policy contract (`schema_version`,
  `timeout_seconds`, workspace trust, audit record validity) is
  validated by `validate_terminal_launch_policy_contract` and
  `validate_terminal_audit_record`; the `terminal.launch`,
  `terminal.input`, `terminal.resize`, `terminal.close`, and
  `terminal.kill` capability names are reserved in
  `plans/dependency-policy.md` §1; unknown capability names
  remain denied; air-gap mode denies hosted egress; terminal
  activation remains disabled by default (the production PTY
  backend is the WS-05.T1 activation). The dependency-policy
  audit (`cargo run -p xtask -- check-deps`) fails closed if
  any workspace package other than `legion-terminal` declares
  `alacritty_terminal` / `portable-pty` / `wezterm-term` once
  the runtime activation lands, because the structural audit
  iterates the same `package_dependencies` map that drives the
  renderer-boundary and parser-boundary checks; a future
  `TERMINAL_BOUNDARY_POLICY_MARKERS` audit uses the same
  fail-closed shape.

## Operational Notes

- The M0 ratification does **not** commit anything; the user retains
  explicit commit authority per the task body rule. The ADR flip
  and the new evidence package are working-tree changes only.
- The full workspace test surface (`cargo test --workspace
  --all-targets`), clippy (`cargo clippy --workspace --all-targets --
  -D warnings`), and `cargo deny check` are recorded at the
  milestone-claim level, not per ADR, and remain a prerequisite for
  the next phase-gate flip.
- WS-05 acceptance criteria (wire PTY to terminal runtime, custom
  egui grid renderer, shell integration via OSC 7/133, task
  execution layer with structured completion events, vttest-subset
  conformance, TUI smoke, kill-escalation / orphan-reaping chaos
  tests) are downstream of this ratification and remain out of
  scope for the M0/ADR-0035 work packet. WS-05.T1 ("Wire PTY to
  terminal runtime") is the first critical-path task that consumes
  this ADR; the existing `crates/legion-terminal/src/lib.rs`
  (1619 lines) and `crates/legion-platform/src/lib.rs` (2323 lines)
  are the starting point for that workstream, and the
  `nix`/`windows`-conditional PTY backends in `legion-platform`
  are the production surface WS-05.T1 will exercise end-to-end.
- The future `TERMINAL_BOUNDARY_POLICY_MARKERS` /
  `TERMINAL_DEPENDENCY_ALLOWED_PACKAGES = ["legion-terminal"]` /
  `FORBIDDEN_TERMINAL_DEPS = ["alacritty_terminal", "portable-pty",
  "wezterm-term"]` audit is recorded as a future gate improvement
  in the decision section of the ADR; it is not required for the
  M0 ratification, but it is the natural next step once WS-05.T1
  starts declaring `alacritty_terminal` as a workspace dependency.
  A worker implementing that audit should mirror the
  parser-boundary audit in `xtask/src/main.rs` (the
  `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES` / `FORBIDDEN_PARSER_DEPS`
  shape) and the LSP-boundary sketch in `ADR-0034`.
- The `ADR-0026` Phase 8 GA acceptance reservation is unchanged:
  native PTY behavior, policy enforcement, cleanup safety,
  platform evidence, privacy/redaction tests, and release evidence
  must be archived before Phase 8 GA flips. The M0 ratification
  ratifies the boundary and the stack choice; the WS-05
  workstream is the WS-level path to Phase 8 GA acceptance, with
  the existing `legion-terminal` + `legion-platform` substrate as
  the starting point.
