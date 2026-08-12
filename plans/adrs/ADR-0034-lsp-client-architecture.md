# ADR-0034: LSP Client Architecture

## Status

Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.

This ADR ratifies the Production Master Plan v0.1 §6 recommendation verbatim
(option (a), hand-rolled stdio JSON-RPC client à la Helix/Zed with
`lsp-types` and a per-language adapter registry), and records the resulting
crate boundary: the LSP runtime is owned by `legion-lsp`, supervised under
the contracts in `ADR-0018-lsp-runtime-supervision.md`, and consumes
`legion-protocol` DTOs only. No amendments to the master-plan recommendation
were required; one small clarification and one deferred-dep note are recorded
below.

## Context

Legion already ships a complete LSP DTO surface and the supervision
contracts from `ADR-0018-lsp-runtime-supervision.md`, but no language server
has ever been launched. WS-03 names rust-analyzer as the flagship language
runtime, with tier-2 servers (TypeScript, Python, Go, …) following once the
core lifecycle is reliable. The architecture must keep four invariants from
the master plan intact:

- **Actor ownership** — LSP workers own server process state, framing, and
  request/response correlation, but never buffers, workspace VFS authority,
  or save authority. (Per `ADR-0018` §1 and `legion-protocol` boundary
  types.)
- **Proposal-mediated write operations** — rename, formatting, code actions,
  and organize imports must materialize as `WorkspaceProposal` payloads
  through the accepted Phase 2 proposal routes (`ADR-0016`); LSP workers
  must never apply edits to buffers or disk.
- **Stale-snapshot protection** — completion, hover, signature help,
  reference, definition, diagnostic, and code-action responses must be
  cancelled or discarded when a newer snapshot supersedes them. The
  supervision contracts in `ADR-0018` §2, §3, and §5 already enforce this.
- **Projection-only UI boundary** — `legion-ui` consumes LSP-derived
  projections; it does not own server processes, document sync, request
  state, or workspace mutation.

The plan compared three options: (a) a hand-rolled stdio JSON-RPC client à
la Helix/Zed with a per-language adapter registry, (b) the `async-lsp`
framework, and (c) `tower-lsp`. Option (a) matches how every shipping Rust
editor did it and fits Legion's existing actor-supervision contracts
without dragging in a new async framework, a Tokio-based runtime, or a
client/server split. The plan explicitly allows `async-lsp` as a fallback
if hand-rolling stalls, but the implementation in `crates/legion-lsp`
already covers framing, request/response correlation, cancellation,
`$/progress`, and supervised lifecycle events, so the fallback is not
needed.

`lsp-types` is named in the plan as the type-level companion to the
hand-rolled client so the wire DTOs and request parameters match the LSP
spec without us redefining them. The current `legion-lsp` crate builds
without `lsp-types` (it only declares `serde`, `serde_json`, `thiserror`,
and `uuid`); adding `lsp-types` is recorded here as a runtime activation
to be done during WS-03 implementation, not as a M0 ratification
prerequisite, because the M0 boundary decision is independent of which
concrete DTO library the runtime adopts.

## Decision

Legion will implement a supervised stdio JSON-RPC LSP client with
`lsp-types` and a per-language adapter registry, all owned by `legion-lsp`
and consumed by `legion-app` through protocol DTOs and projections.

- **Runtime ownership.** `legion-lsp` owns the JSON-RPC framing, the
  request/response correlation table, cancellation tokens, `$/progress`
  routing, server process lifecycle (spawn, health states, crash detection,
  bounded restart with backoff, circuit-breaker), and a per-language
  adapter registry. The implementation already lives at
  `crates/legion-lsp/src/lib.rs` (process spawn, `LspRuntimeError` taxonomy,
  `LspRequestId` correlation, supervised lifecycle event types, and the
  framing state machine) and is contracted by the four test files under
  `crates/legion-lsp/tests/`: `stdio_transport_contract.rs`,
  `lifecycle_contract.rs`, `document_sync_contract.rs`, and
  `read_side_contract.rs`.
- **`lsp-types` adoption.** `legion-lsp` will add `lsp-types` to its
  workspace dependency set during WS-03.T1 (or later, when the first
  typed request parameter needs to round-trip through the framing layer).
  The dependency is a runtime activation under
  `plans/dependency-policy.md` §1 (`legion-lsp` is the only workspace
  crate allowed to declare it, alongside `legion-observability`,
  `legion-platform`, `legion-protocol`, `legion-security`,
  `legion-storage`, and a `legion-text` dev/test-only entry for UTF-16
  coordinate conformance). The M0 ratification does not require the
  dependency to be wired today; it only commits the boundary that says
  `legion-lsp` is the only crate allowed to introduce it.
- **Per-language adapter registry.** The adapter registry is a
  `legion-lsp`–internal dispatch table keyed by `LanguageServerId`
  (already an authoritative boundary symbol in `legion-protocol`) and
  `LanguageId`. The registry is populated by configuration, not by code
  generation, and is the only path through which a new language can be
  wired in. The registry does not own protocol DTOs, proposals, editor
  sessions, workspace state, or save authority.
- **Read-side vs. write-side split.** Read-side features (completion,
  hover, signature help, go-to-def/decl/impl/type-def, find references,
  document/workspace symbols, semantic tokens, inlay hints, folding
  ranges, diagnostics publication) may update projections directly
  through `legion-app` composition, but the runtime still tags every
  result with `CorrelationId`, `CausalityId`, `SnapshotId`,
  `BufferVersion`, freshness, and capability provenance so UI can mark
  delayed/stale/degraded/unavailable state per `ADR-0018` §4–5. Write-side
  features (rename, formatting, code actions, organize imports, quick
  fixes, refactor actions) must materialize as `WorkspaceProposal`
  payloads through the existing `legion-app` proposal routes; LSP
  workers never apply edits to buffers or disk.
- **rust-analyzer extensions in scope.** For the flagship language
  runtime, the protocol extensions for flycheck, runnables, inlay hints,
  macro expansion, and open-docs are in-scope for WS-03. They ride on the
  same framing/correlation/lifecycle contracts as standard LSP and must
  obey the same read-side / write-side split; flycheck and runnables are
  read-side metadata, and macro-expansion / open-docs read paths are
  read-side navigation; the write-side entries flow through proposals.
- **Crate boundary.** `legion-lsp` is the only workspace crate authorized
  to declare LSP runtime dependencies. `legion-app` composes LSP outputs
  through protocol DTOs and the proposal route; `legion-index` keeps
  lexical and tree-sitter state and may consume LSP-derived semantic
  tokens and diagnostics through protocol DTOs only; `legion-editor`,
  `legion-ui`, `legion-desktop`, `legion-project`, `legion-storage`,
  `legion-ai`, `legion-agent`, `legion-remote`, `legion-plugin`,
  `legion-collaboration`, `legion-tracker`, and `legion-memory` must
  never declare `lsp-types` or any LSP transport / framing dependency.
  This boundary mirrors the parser-boundary gate in
  `ADR-0033-syntax-parse-engine.md` and is enforced by the same
  `cargo run -p xtask -- check-deps` policy-text + package-dependency
  audit (the `PARSER_BOUNDARY_POLICY_MARKERS` and
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES` constants in `xtask/src/main.rs`
  become the model for a future `LSP_BOUNDARY_POLICY_MARKERS` /
  `LSP_DEPENDENCY_ALLOWED_PACKAGES = ["legion-lsp"]` /
  `FORBIDDEN_LSP_DEPS = ["lsp-types", "async-lsp", "tower-lsp"]`
  extension, but no new `xtask` subcommand is required for M0).
- **Proposal-mediated mutation (unchanged from `ADR-0018` §7).** LSP
  workers translate edit-producing responses into `WorkspaceProposal`
  payloads with explicit target coverage, version preconditions, privacy
  metadata, capability requirements, rollback expectations, and preview
  summaries. The proposal service validates, previews, approves, applies,
  rejects, cancels, or rolls back these proposals. LSP workers never
  apply edits directly to buffers or disk. If a proposal cannot express
  the server response safely, the action is denied with metadata-only
  diagnostics rather than applying a partial edit.

## Consequences

- **Positive:** the architecture matches proven Rust-editor patterns
  (Helix / Zed / rust-analyzer consumer idioms) while fitting Legion's
  actor-supervision contracts (`ADR-0018`) and the read/write split
  already enforced by the Phase 2 proposal routes (`ADR-0016`).
- **Positive:** `legion-lsp` already exists with framing, correlation,
  cancellation, and supervised lifecycle scaffolding; the M0 ratification
  ratifies a working boundary rather than a future build, and the
  downstream WS-03 tasks can build on top of the existing contract
  tests in `crates/legion-lsp/tests/`.
- **Positive:** tier-2 server onboarding becomes a registry entry
  (configuration, not code generation) and obeys the same per-language
  adapter invariants as the flagship.
- **Negative:** hand-rolling requires careful protocol conformance and
  backpressure handling. The `ADR-0018` §4–5 backpressure and
  supervision contracts plus the four existing contract test files in
  `crates/legion-lsp/tests/` cover the known failure modes, but
  protocol edge cases (e.g. server-initiated `$/cancelRequest`,
  `workspace/configuration`, `client/registerCapability`) will need
  typed DTOs and contract tests as they activate.
- **Negative:** introducing `lsp-types` is a runtime activation that
  must respect the dependency policy. Until then, the runtime speaks
  raw JSON via `serde`/`serde_json` and protocol DTOs come from
  `legion-protocol`'s hand-rolled types.
- **Mitigation:** start with the scripted mock-server suite in
  `crates/legion-lsp/tests/stdio_transport_contract.rs` and
  `lifecycle_contract.rs` (already present and passing), then add a
  rust-analyzer live smoke test against the Legion repo in WS-03
  (M1 evidence target).

## Verification

- `cargo run -p xtask -- check-deps` (dependency direction, structural
  audit, protocol contract, and phase 3/4/5/6/7/8/13 acceptance
  governance) — confirms `legion-lsp`'s manifest matches the
  `legion-lsp` policy entry in `plans/dependency-policy.md` §1 (allowed
  internal edges and the WS-03.T1 activation scope), and that no other
  workspace crate declares `lsp-types` / `async-lsp` / `tower-lsp` once
  the runtime activation lands.
- `cargo run -p xtask -- docs-hygiene` (broken relative Markdown links
  and the unallowlisted stale Legion-rename marker)
- `cargo run -p xtask -- no-egui-textedit` (companion gate, unchanged)
- `cargo fmt --all --check`
- `cargo check -p xtask`
- `cargo test -p legion-lsp --tests` (stdio transport, lifecycle,
  document sync, and read-side contract tests in
  `crates/legion-lsp/tests/`)
- WS-03 evidence under `plans/evidence/production/m1/` once the LSP
  runtime productizes (out of scope for this M0 ratification).
