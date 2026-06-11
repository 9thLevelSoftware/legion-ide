# ADR-0035: Terminal Stack

## Status

Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.

This ADR ratifies the Production Master Plan v0.1 §6 recommendation verbatim
(option (a), `alacritty_terminal` VTE grid + the existing `legion-platform`
PTY layer with a custom egui terminal renderer), and records the resulting
crate boundary: the terminal runtime is owned by `legion-terminal` over the
`legion-platform` PTY/process layer and consumed by `legion-app` through
projection DTOs and the policy-gated launch path. `alacritty_terminal` is
recorded as a WS-05.T1 runtime activation, not an M0 dependency manifest
prerequisite, because the M0 boundary decision is independent of which
concrete VTE/grid library the runtime adopts.

## Context

Legion has already audited the cross-platform PTY/process primitives in
`legion-platform` (`crates/legion-platform/src/lib.rs`, 2323 lines:
`openpty`/ConPTY spawn, process tree, kill escalation, signal handling,
filesystem/watch/environment/time operations) and the protocol-level
terminal DTO surface in `legion-protocol` (terminal session, launch
policy, audit, input/resize/close/kill, command-block metadata, OSC 7/133
shape). `legion-terminal` (`crates/legion-terminal/src/lib.rs`, 1619
lines) implements the deterministic `TerminalFixtureRuntime` backend,
`DapAdapterFixtureRuntime` (debug-protocol fixture), launch-policy
validation, and terminal audit record emission per `ADR-0026`'s
acceptance reservation. The app-level terminal execution still uses the
deterministic fixture by default — `fixture_enabled` defaults to false in
beta per `plans/legion-production-master-plan-v0.1.md` §3.2 — and the
real PTY path is unwired. WS-05 ("Terminal Runtime (real PTY)") is the
workstream that activates the integrated terminal and the agent-consumable
structured command output; the agent harness will consume command-block
metadata as a core differentiator (per §1, §4 of the master plan).

Four invariants from the master plan §2.2 constrain the terminal stack:

- **App-composed and capability-gated** — terminal launch is a security
  broker decision (`terminal.launch`, `terminal.input`, `terminal.resize`,
  `terminal.close`, `terminal.kill` are reserved capability names in
  `plans/dependency-policy.md` §1). The terminal crate may not bypass the
  broker, may not mutate workspace/editor/disk, and may not persist raw
  command bodies, transcripts, process output, secrets, or full
  environment values.
- **Proposal-mediated mutation** — any terminal-originated mutation
  candidate must materialize as a `WorkspaceProposal` payload through the
  accepted Phase 2 proposal routes (`ADR-0016`); the terminal runtime
  itself never applies edits to buffers or disk.
- **Projection-only UI boundary** — `legion-ui` consumes terminal
  projections, emits `CommandDispatchIntent`, and never owns terminal
  session state, PTY/process state, or mutation authority (per
  `AGENTS.md` and `plans/dependency-policy.md` §1).
- **Metadata-first observability** — terminal sessions emit
  metadata-only audit records per `legion-protocol`'s
  `TerminalAuditRecord` contract, with `CorrelationId` /
  `CausalityId` / `EventSequence` and capability provenance, and the
  observability sinks reject zero/empty IDs as for every other crate.

The plan compared three options: (a) `alacritty_terminal` VTE grid +
`legion-platform` PTY with a custom egui renderer, (b) `alacritty_terminal`
+ `portable-pty` (a second audited PTY implementation in parallel with
`legion-platform`), and (c) `wezterm-term`. Option (a) reuses the
already-audited `legion-platform` PTY/process layer (which already passes
the policy gate, has real ConPTY/nix `openpty` backends wired, and is
covered by platform tests), avoids introducing a second audited PTY
implementation in parallel, and matches how Zed and Helix wire their
terminal stacks (their renderer is custom, their grid model comes from
`alacritty_terminal`, their PTY/process layer is their own). Option (b)
would duplicate `legion-platform`'s audit surface and conflict with the
"reuse audited primitives" rule in `ADR-0026`. Option (c) would drag in
a different grid model and a much larger dependency footprint
(`wezterm-term` is part of the WezTerm monorepo and ships with its own
PTY/event-loop scaffolding that would conflict with `legion-platform`'s
boundary).

## Decision

Legion will use `legion-platform` for PTY/process ownership and policy
gates, `legion-terminal` for terminal session state and protocol DTOs,
`alacritty_terminal` for the VTE/grid model, and a custom egui renderer
painted by `legion-app` from projected terminal state.

- **Runtime ownership.** `legion-terminal` owns the terminal session
  lifecycle, fixture-vs-real runtime switch, launch policy validation,
  terminal audit record emission, and protocol DTOs. The runtime is
  already wired for the deterministic `TerminalFixtureRuntime` and
  `DapAdapterFixtureRuntime` backends; the production PTY backend is the
  WS-05.T1 activation that drives `legion-platform` PTY primitives
  (spawn, resize, input, kill escalation) and feeds the `alacritty_terminal`
  `Term` grid. The launch / input / resize / close / kill semantics,
  cwd/env/shell policy, output chunking, truncation, timeout, kill-tree
  cleanup, and orphan detection are defined by the existing
  `TerminalFixtureRuntime` and `validate_terminal_*` functions in
  `crates/legion-terminal/src/lib.rs`, and the production PTY backend
  reuses the same validation surface (the fixture already models the
  policy contract, not just the data shape).
- **Platform layer.** `legion-platform` owns the cross-platform
  filesystem/process/watcher/PTY/environment/time operations. Its
  current `Cargo.toml` declares `legion-protocol` and `thiserror`, plus
  the platform-conditional `nix` (Unix PTY/process/signal handling) and
  `windows` (ConPTY: `Win32_System_Console`, pipes, processes,
  foundation, file system, security, IO) — matching the Phase 8
  production dependency rebaseline in `plans/dependency-policy.md` §1,
  which authorizes `nix` or `rustix` for Unix PTY and `windows` for
  ConPTY in `legion-platform` and `legion-terminal`. The platform
  process/PTY layer is the audited boundary; the terminal crate and the
  app layer never re-implement process ownership.
- **VTE/grid model (`alacritty_terminal`).** `alacritty_terminal` is the
  VTE/grid library used by Alacritty itself, factored out as a
  standalone Rust crate, and adopted by Zed, Helix, and several other
  Rust editors for the same reason Legion adopts it: it gives a
  production-grade grid state model with deterministic styles, cursor
  tracking, scrollback, OSC parsing (including OSC 7 cwd tracking and
  OSC 133 prompt marking), and selection semantics, without dragging in
  a renderer or an event loop. Legion uses it as a state library only;
  the egui renderer is custom and paints terminal cells/styled
  cursor/selection/scrollback/URL detection from the projected grid
  state per WS-05.T2. The dependency is added during WS-05.T1 as a
  runtime activation under `plans/dependency-policy.md` §1 (the same
  runtime-activation pattern ADR-0033 records for tree-sitter and
  ADR-0034 records for `lsp-types`).
- **Custom egui renderer.** The terminal grid is rendered by a custom
  egui widget in `legion-app` / `legion-desktop` (12px terminal type
  per design tokens, vttest-subset conformance target, TUI-class
  usability per WS-05.T2). The renderer never owns terminal session
  state, never owns PTY/process state, and never owns mutation
  authority; it paints projections emitted by `legion-app`. The
  "no `egui::TextEdit` in the code canvas" rule from ADR-0032 still
  applies to terminal grid rendering; the `xtask no-egui-textedit`
  gate is the companion check.
- **Shell integration (OSC 7/133).** OSC 133 prompt marking (command
  boundaries, per-command exit status, duration display) and OSC 7
  cwd tracking are product requirements because structured terminal
  output is the agent-harness differentiator (per the master plan §1
  and §4). The OSC parser lives in the terminal runtime; the
  command-block metadata (command, exit code, duration, bounded output
  reference, `CorrelationId`, `CausalityId`) is emitted as a
  `TerminalAuditRecord`/`CommandBlockMetadata` projection. UI uses
  command blocks to navigate between commands, attach exit codes, and
  surface duration; agents consume the same metadata as harness tool
  output (per the master plan §4 "structured terminal output feeds the
  agent harness — a real differentiator").
- **Crate boundary.** `legion-terminal` is the only workspace crate
  authorized to declare `alacritty_terminal` (or any VTE/grid
  dependency) and the only workspace crate authorized to drive the
  `legion-platform` PTY layer into the terminal session lifecycle.
  `legion-app` composes terminal outputs through protocol DTOs and
  policy-gated launch paths; `legion-ui` consumes terminal projections
  and emits `CommandDispatchIntent` only; `legion-editor`,
  `legion-desktop`, `legion-project`, `legion-storage`, `legion-ai`,
  `legion-agent`, `legion-remote`, `legion-plugin`,
  `legion-collaboration`, `legion-tracker`, `legion-memory`, and
  `legion-index` must never declare `alacritty_terminal` or any
  PTY/terminal runtime dependency. This boundary mirrors the
  parser-boundary gate in `ADR-0033` and the LSP-boundary sketch in
  `ADR-0034` and is enforced by the same `cargo run -p xtask --
  check-deps` policy-text + package-dependency audit. A future
  `TERMINAL_BOUNDARY_POLICY_MARKERS` /
  `TERMINAL_DEPENDENCY_ALLOWED_PACKAGES = ["legion-terminal"]` /
  `FORBIDDEN_TERMINAL_DEPS = ["alacritty_terminal", "portable-pty",
  "wezterm-term"]` audit is sketched in the decision for the next
  phase, but no new `xtask` subcommand is required for M0 because
  the `alacritty_terminal` dependency is a WS-05.T1 runtime
  activation, not a M0 manifest change.
- **Proposal-mediated mutation (unchanged from `ADR-0026`).** The
  terminal runtime never mutates the workspace, editor, or disk
  directly. Any terminal-originated mutation candidate (a build/test
  task that needs to record structured completion, a command that
  resolves to a file edit, an agent tool result that should be
  retained) materializes as a `WorkspaceProposal` payload through the
  existing `legion-app` proposal routes. The terminal runtime reports
  structured command-block metadata so the proposal service can
  preview, approve, apply, reject, cancel, or roll back. The fixture
  audit records already model the policy contract, and the production
  PTY backend reuses the same `TerminalAuditRecord` / `CommandBlock`
  validation surface.
- **Metadata-only persistence (unchanged from `ADR-0026`).** Terminal
  sessions persist metadata-only summaries by default: session id,
  launch policy contract id, command boundaries, exit code, duration,
  bounded output reference (truncated, redacted), `CorrelationId`,
  `CausalityId`. Raw command bodies, transcripts, full process
  output, secrets, and full environment values are never persisted.
  This is the privacy/redaction rule in `ADR-0026` and the
  metadata-first observability invariant in the master plan §2.2.
  Audit records are validated by the existing
  `validate_terminal_audit_record` function in
  `crates/legion-terminal/src/lib.rs`.
- **Cleanup and orphan safety (unchanged from `ADR-0026`).** The
  terminal runtime is cleanup-safe: kill escalation models the full
  SIGINT → SIGTERM → SIGKILL tree, process groups are tracked through
  the `legion-platform` layer, orphan reaping is part of the platform
  process surface, and termination is bounded by the launch policy's
  `timeout_seconds` field. The platform layer's process/PTY boundary
  is the audited surface; the terminal runtime reuses the platform
  cleanup contract and adds a runtime-side test in WS-05.T1.
- **Compatibility with `ADR-0026`'s acceptance reservation.** The
  Phase 8 GA acceptance for the terminal runtime remains blocked in
  `ADR-0026` until native PTY behavior, policy enforcement, cleanup
  safety, platform evidence, privacy/redaction tests, and release
  evidence are archived. The M0 ratification does not flip Phase 8
  GA — it ratifies the boundary and the stack choice, and WS-05.T1
  is the WS-level activation that drives the production PTY backend
  end-to-end.

## Consequences

- **Positive:** reuses the audited `legion-platform` PTY/process layer
  (no second audited PTY implementation, no audit duplication), gets
  a production-grade grid model from `alacritty_terminal` (the same
  library that ships in Alacritty/Zed/Helix), and renders through the
  custom egui grid pattern that already fits the projection-only UI
  boundary. The agent-harness differentiator (structured command
  blocks with exit code, duration, bounded output reference) is a
  product requirement, not a future possibility.
- **Positive:** the M0 ratification ratifies a working boundary and
  an already-shipped substrate. The deterministic fixture backend
  (`TerminalFixtureRuntime`/`DapAdapterFixtureRuntime`), the launch
  policy validation, the audit record emission, and the `nix`/ConPTY
  platform backends are all live in the current tree. WS-05 has a
  real starting point, not a future build.
- **Positive:** tier-2 shell integrations (fish, nushell, pwsh,
  cmd.exe, etc.) become a runtime configuration + per-shell OSC
  adapter, not a code-generation step. The OSC 7/133 contract is the
  boundary; the per-shell quirks stay in the terminal runtime.
- **Negative:** custom rendering adds cross-platform complexity
  (font rasterization, glyph metrics, color styles, scrollback
  semantics, URL detection, IME). The existing `legion-editor`
  text-shape and `legion-app` theme/token plumbing are the natural
  reuse points, but the terminal grid has its own coordinate system
  (cells vs. bytes) that the renderer has to handle.
- **Negative:** introducing `alacritty_terminal` is a runtime
  activation that must respect the dependency policy. Until then,
  the terminal runtime speaks to the deterministic fixture and the
  production PTY backend is the WS-05.T1 work item.
- **Mitigation:** vttest-subset conformance + TUI smoke evidence are
  the WS-05.T2 acceptance criteria, chaos tests for orphan cleanup
  are the WS-05.T1 acceptance criteria, and the existing
  `crates/legion-terminal/tests/dap_adapter_fixture.rs` and the
  `TerminalFixtureRuntime` unit tests in
  `crates/legion-terminal/src/lib.rs` are the M0 test surface that
  WS-05 will extend.

## Verification

- `cargo run -p xtask -- check-deps` (dependency direction + structural
  audit, with the `legion-terminal` and `legion-platform` policy
  entries verified against `plans/dependency-policy.md` §1 and the
  Phase 8 production dependency rebaseline)
- `cargo run -p xtask -- docs-hygiene` (broken relative Markdown links
  and the unallowlisted stale Legion-rename marker)
- `cargo run -p xtask -- no-egui-textedit` (companion gate, unchanged
  from ADR-0032; the terminal grid renderer is a custom egui widget,
  not an `egui::TextEdit`)
- `cargo fmt --all --check`
- `cargo test -p legion-terminal --tests` (deterministic fixture
  backend, launch policy validation, audit record emission, and the
  DAP adapter fixture contract test)
- `cargo test -p legion-platform --tests` (filesystem/process/PTY/
  watcher/environment/time surface that backs the terminal runtime)
- WS-05 evidence under `plans/evidence/production/m1/` once the
  production PTY backend is product-validated (out of scope for this
  M0 ratification)
