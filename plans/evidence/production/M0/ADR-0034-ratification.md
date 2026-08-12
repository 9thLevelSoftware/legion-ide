# M0 — ADR-0034 (LSP Client Architecture) Ratification Evidence

Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
ADR: [`plans/adrs/ADR-0034-lsp-client-architecture.md`](../../../adrs/ADR-0034-lsp-client-architecture.md)
Date: 2026-06-10
Gate: `cargo run -p xtask -- check-deps` (dependency direction + structural
audit, with `legion-lsp` policy entry verified against
`plans/dependency-policy.md` §1 and the WS-03.T1 activation scope)
Acceptance target: master-plan §6 row "ADR-0034 | LSP client architecture"
→ option (a) ratified in-repo, with the LSP runtime owned by `legion-lsp`
and consumed through `legion-app`/`legion-protocol` only.

## Decision Recorded

- Status flipped from `Draft` to `Accepted` in
  `plans/adrs/ADR-0034-lsp-client-architecture.md`.
- Decision text matches Production Master Plan v0.1 §6 recommendation
  verbatim: option (a) hand-rolled stdio JSON-RPC client à la Helix/Zed
  with `lsp-types` and a per-language adapter registry, fitting Legion's
  existing actor-supervision contracts from `ADR-0018`. `async-lsp` remains
  the documented fallback if hand-rolling stalls; the existing
  implementation in `crates/legion-lsp` does not require that fallback.
- No amendments were required to the master-plan recommendation. The ADR
  adds three confirmations consistent with the plan and with current
  code/contracts:
  1. The runtime ownership is `legion-lsp` and is already an active crate
     (`crates/legion-lsp/src/lib.rs`, 2108 lines, plus the four contract
     test files under `crates/legion-lsp/tests/`: `stdio_transport_contract.rs`,
     `lifecycle_contract.rs`, `document_sync_contract.rs`, and
     `read_side_contract.rs`). The M0 ratification ratifies a working
     boundary, not a future build.
  2. The `lsp-types` dependency is recorded as a runtime activation to
     be done during WS-03.T1 (or later, when the first typed request
     parameter needs to round-trip). The M0 ratification commits only to
     the boundary: `legion-lsp` is the only workspace crate allowed to
     declare `lsp-types`, `async-lsp`, or `tower-lsp`.
  3. The read-side / write-side split is spelled out explicitly so the
     downstream WS-03 workstreams can build on the existing Phase 2
     proposal routes (`ADR-0016`) and the supervision contracts in
     `ADR-0018` §1–§8 without re-litigating the boundary.

## Crate / Dependency Boundary Impact

- No new internal crate edges are introduced by this ADR.
- The `legion-lsp` policy entry in `plans/dependency-policy.md` §1 is
  unchanged: `legion-lsp` may depend on `legion-observability`,
  `legion-platform`, `legion-protocol`, `legion-security`,
  `legion-storage`, and `legion-text` (dev/test-only for UTF-16
  coordinate conformance until WS03.T2 decides the runtime composition
  boundary). The current `crates/legion-lsp/Cargo.toml` declares only
  `legion-protocol`, `serde`, `serde_json`, `thiserror`, and `uuid`
  (plus `legion-text` as a dev-dependency) — a subset of the
  policy-allowed set, so the ratification does not require any manifest
  change today.
- The `legion-app` policy entry already lists `legion-lsp` as an allowed
  internal dependency (line 93 of `plans/dependency-policy.md` §1); no
  policy change is required.
- The `lsp-types` workspace dependency is **not** added to the root
  `Cargo.toml` at M0. It will be added during WS-03.T1 as a runtime
  activation under the same dependency-policy gate that authorized the
  parser-boundary audit in `ADR-0033`. The gate is forward-compatible
  with a future `LSP_BOUNDARY_POLICY_MARKERS` /
  `LSP_DEPENDENCY_ALLOWED_PACKAGES = ["legion-lsp"]` /
  `FORBIDDEN_LSP_DEPS = ["lsp-types", "async-lsp", "tower-lsp"]` audit
  shaped like the existing `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]` /
  `FORBIDDEN_PARSER_DEPS = ["tree-sitter", "tree-sitter-rust"]` audit
  in `xtask/src/main.rs:446-453`. The M0 ratification does not require
  the LSP audit to land today; the ADR commits to the boundary and to
  the runtime activation path, not to a new `xtask` subcommand.
- `xtask` does not need a new subcommand. The structural dependency
  audit and the protocol-contract audit that already run as part of
  `check-deps` are sufficient to enforce the current `legion-lsp` policy
  entry; the future LSP-boundary audit is a phase-gate improvement, not
  an M0 prerequisite.

## Gate Evidence (verbatim)

All gates were run against the current working tree with commit baseline
`b56dcb2`; the ratification changes (ADR flip + this evidence file) are
untracked as required by the task's "no commit without explicit user
instruction" rule.

### `cargo run -p xtask -- check-deps`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- check-deps
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/xtask check-deps`
dependency policy checks passed
```

Exit code: `0`. The renderer-boundary audit, the parser-boundary audit,
the structural dependency audit, the protocol-contract audit, and the
phase 3/4/5/6/7/8/13 acceptance governance audits all pass against the
current tree. In particular:

- `plans/dependency-policy.md` still contains every
  `PARSER_BOUNDARY_POLICY_MARKERS` string (`xtask/src/main.rs:446-451`).
- The `legion-lsp` policy entry at lines 182-190 is intact and matches
  `crates/legion-lsp/Cargo.toml` (a subset of the allowed internal
  edges, no LSP transport / framing dependency declared today).
- The structural audit confirms `legion-app`'s `legion-lsp` edge is
  policy-allowed and `legion-editor` does not declare a `legion-lsp`
  edge (the `legion-editor` policy entry at lines 43-52 only authorizes
  `legion-observability`, `legion-protocol`, and `legion-text`).

### `cargo run -p xtask -- docs-hygiene`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- docs-hygiene
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/xtask docs-hygiene`
documentation hygiene checks passed
```

Exit code: `0`. Confirms the ADR-0034 ratification does not break
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
did not touch the painter module or its scanned paths.

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

### `cargo check -p xtask`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo check -p xtask
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.90s
```

Exit code: `0`. Confirms the `xtask` crate still builds cleanly with
the parser-boundary markers and audit logic unchanged; the
documentation-only ratification did not touch the gate itself.

### `cargo test -p legion-lsp --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-lsp --tests 2>&1 | grep -E '^(test result|running)'
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
running 20 tests
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Exit code: `0`. Across the five test binaries (the crate's `lib.rs`
unittests are split into two empty stubs for layout, plus the four
contract test files the ADR cites), **41 contract tests pass with 0
failures**. Highlights:

- `stdio_transport_contract.rs` (8 tests): mock-server initialization,
  process reuse, `$/progress` notification routing, `$/cancelRequest`
  propagation + late-response rejection, policy-deny-without-spawn,
  diagnostic notification metadata capture, and the
  `rust_analyzer_initializes_against_legion_repo_when_opted_in` live
  smoke that WS-03.T1 will productize.
- `lifecycle_contract.rs` (10 tests): supervisor starts a running
  server, rejects untrusted workspaces without spawning, recovers from
  crash with bounded backoff, plus framing primitives
  (case-insensitive `Content-Length`, oversized payload rejection,
  round-trip, out-of-order response correlation, JSON-RPC error mapping
  to `Unavailable`, and timeout-after-budget).
- `read_side_contract.rs` (20 tests): completion / hover / definition /
  references / document symbols / workspace symbols / inlay hints /
  code-lens request shapes (UTF-16 positions, document URI, context
  fields), response projections, bounded completion rows, malformed
  response handling.
- `document_sync_contract.rs` (3 tests): `didOpen` plus incremental
  `didChange` with UTF-16 ranges.

The M0 ratification ratifies a working boundary rather than a future
build.

## Invariant Preservation Checklist

- [x] Projection-only UI: `legion-ui` still emits `CommandDispatchIntent`
  and accepts snapshots; LSP transport / framing crates stay out of
  `legion-ui` per the dependency policy. The read-side / write-side
  split is now spelled out in the ADR so downstream WS-03 workstreams
  cannot regress the projection boundary. Unchanged.
- [x] Proposal-mediated mutation: reaffirmed in the ADR's
  "Proposal-mediated mutation (unchanged from `ADR-0018` §7)"
  paragraph. Write-side features (rename, formatting, code actions,
  organize imports) must materialize as `WorkspaceProposal` payloads
  through the existing `legion-app` proposal routes; LSP workers never
  apply edits to buffers or disk. The Phase 2 proposal routes
  (`ADR-0016`) and the supervision contracts (`ADR-0018` §1, §2, §7)
  are unchanged.
- [x] Metadata-first observability: reaffirmed through the read-side
  contract — every read-side result carries `CorrelationId`,
  `CausalityId`, `SnapshotId`, `BufferVersion`, freshness, and
  capability provenance so UI can mark delayed/stale/degraded/
  unavailable state per `ADR-0018` §4–5. The observability sinks that
  reject zero `CorrelationId` / nil `CausalityId` / zero
  `EventSequence` already apply to LSP output via the existing
  `legion-lsp` correlation table; no new crate edge is required.
- [x] Fail-closed policy: enforced at the supervision boundary
  (`ADR-0018` §5 — timeout results are typed outcomes, restart storms
  trip a circuit breaker, server crashes / malformed responses /
  unsupported capabilities are isolated to the supervised worker and
  must not corrupt editor state, save state, or index state). The
  dependency-policy audit (`cargo run -p xtask -- check-deps`) fails
  closed if any workspace package other than `legion-lsp` declares
  `lsp-types` / `async-lsp` / `tower-lsp` once those dependencies are
  introduced, because the structural audit iterates the same
  `package_dependencies` map that drives the renderer-boundary and
  parser-boundary checks; a future `LSP_BOUNDARY_POLICY_MARKERS` audit
  uses the same fail-closed shape.

## Operational Notes

- The M0 ratification does **not** commit anything; the user retains
  explicit commit authority per the task body rule. The ADR flip and
  the new evidence package are working-tree changes only.
- The full workspace test surface (`cargo test --workspace --all-targets`),
  clippy (`cargo clippy --workspace --all-targets -- -D warnings`), and
  `cargo deny check` are recorded at the milestone-claim level, not per
  ADR, and remain a prerequisite for the next phase-gate flip.
- WS-03 acceptance criteria (transport + lifecycle, document sync +
  diagnostics, read-side features, write-side features through
  proposals, runnables/flycheck/inlay hints, mock-server contract
  tests, rust-analyzer live smoke) are downstream of this
  ratification and remain out of scope for the M0/ADR-0034 work packet.
  WS03.T1 ("LSP transport + lifecycle") is the first critical-path
  task that consumes this ADR; the existing
  `crates/legion-lsp/src/lib.rs` and the four contract test files in
  `crates/legion-lsp/tests/` are the starting point for that workstream.
- The future `LSP_BOUNDARY_POLICY_MARKERS` /
  `LSP_DEPENDENCY_ALLOWED_PACKAGES = ["legion-lsp"]` /
  `FORBIDDEN_LSP_DEPS = ["lsp-types", "async-lsp", "tower-lsp"]`
  audit is recorded as a future gate improvement in the decision
  section of the ADR; it is not required for the M0 ratification, but
  it is the natural next step once WS-03.T1 starts declaring
  `lsp-types` as a workspace dependency. A worker implementing that
  audit should mirror the parser-boundary audit at
  `xtask/src/main.rs:1045-1091`.
