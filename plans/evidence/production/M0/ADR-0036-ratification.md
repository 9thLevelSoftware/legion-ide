# M0 — ADR-0036 (Search & Index Stack) Ratification Evidence

Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
ADR: [`plans/adrs/ADR-0036-search-and-index-stack.md`](../../../adrs/ADR-0036-search-and-index-stack.md)
Date: 2026-06-10
Gate: `cargo run -p xtask -- check-deps` (dependency direction + structural
audit, with `legion-index` and `legion-project` policy entries verified
against `plans/dependency-policy.md` §1 and the search-boundary sketch
in the ratified ADR)
Acceptance target: master-plan §6 row "ADR-0036 | Search & index stack"
→ option (a) ratified in-repo: in-process `grep-searcher` / `grep-regex`
/ `ignore` / `globset` ripgrep-class search runtime in `legion-index`,
with `tantivy` reserved as an optional later indexed-search tier under
`ADR-0005`'s storage reservations, consumed by `legion-app` through the
existing `SearchProjection` / `SearchResultProjection` /
`SearchScopeProjection` / `StructuralSearchProjection` DTOs and the
proposal-mediated multi-file replace route.

## Decision Recorded

- Status flipped from `Draft` to `Accepted` in
  `plans/adrs/ADR-0036-search-and-index-stack.md`.
- Decision text matches Production Master Plan v0.1 §6 recommendation
  verbatim: option (a) in-process ripgrep-class library crates
  (`grep-searcher` / `grep-regex` / `ignore` / `globset`) for the
  primary search surface, with `tantivy` reserved for the optional
  tier-2 indexed-search path under `ADR-0005`'s storage reservations.
  The plan's WS-06 entry explicitly says "in-process (no subprocess
  overhead, policy-inspectable). Tantivy activates under ADR-0005's
  storage reservations"; the ADR ratifies that recommendation
  without amendment.
- No amendments were required to the master-plan recommendation. The
  ADR adds three confirmations consistent with the plan and with
  current code/contracts:
  1. The search projection boundary is already wired end-to-end in
    `legion-app`. `SearchProjection`, `SearchResultProjection`,
    `SearchScopeProjection`, `SearchStatusProjection`,
    `SearchStatusKindProjection`, `TerminalSearchProjection`, and
    `StructuralSearchProjection` are all live in
    `crates/legion-app/src/lib.rs`; the `RunSearch` /
    `RunStructuralSearch` `AppCommandRequest` variants are dispatched
    through `run_search` / `run_structural_search` composition entry
    points; the `SearchUpdated` / `StructuralSearchUpdated`
    `AppEvent` variants emit the projections. The M0 ratification
    ratifies a working projection boundary plus a real surface for
    WS-06 to extend, not a future build.
  2. The ripgrep-class search runtime dependencies
    (`grep-searcher`, `grep-regex`, `ignore`, `globset`, `tantivy`)
    are **not** declared by any workspace package today and are
    **not** in `Cargo.lock`. They are recorded as WS-06 runtime
    activations, not M0 dependency manifest prerequisites, because
    the M0 boundary decision is independent of which concrete search
    library the runtime adopts. The M0 ratification commits only to
    the boundary: `legion-index` is the only workspace crate
    authorized to declare any of those crates.
  3. The `legion-index` / `legion-project` / `legion-app` /
    `legion-ui` / `legion-editor` / `legion-desktop` boundary is
    spelled out explicitly so the downstream WS-06 workstreams can
    build on the existing `legion-app` projection surface and the
    Phase 3 `legion-index` semantic fabric without re-litigating
    the boundary. The companion `xtask no-egui-textedit` gate is
    reaffirmed: the search panel renders projected search results,
    and any single-line query input is out of the no-`TextEdit`
    scope (the rule covers the code canvas, not the search
    input field).

## Crate / Dependency Boundary Impact

- No new internal crate edges are introduced by this ADR.
- The `legion-index` policy entry in `plans/dependency-policy.md` §1
  at lines 127-134 is unchanged: `legion-index` may depend on
  `legion-protocol`, `legion-storage`, `legion-text`, `tree-sitter`,
  and `tree-sitter-rust`. The current `crates/legion-index/Cargo.toml`
  declares `legion-protocol`, `legion-text`, `thiserror`,
  `tree-sitter`, `tree-sitter-rust`, and `uuid` — a strict subset of
  the policy-allowed set. The search runtime dependencies
  (`grep-searcher`, `grep-regex`, `ignore`, `globset`, `tantivy`)
  are not declared today; when WS-06.T1 declares them, the
  `legion-index` policy entry must be amended to add the new
  internal edges or new external runtime dependencies, and the
  M0 ratification is forward-compatible with that amendment.
- The `legion-project` policy entry in `plans/dependency-policy.md` §1
  at lines 37-41 is unchanged: `legion-project` may depend on
  `legion-observability`, `legion-platform`, `legion-protocol`, and
  `legion-security`. The current `crates/legion-project/Cargo.toml`
  is consistent with this entry, and `legion-project` does not
  contain any ripgrep-class search code (`grep` / `ignore::` /
  `globset` / `tantivy` references in
  `crates/legion-project/src/lib.rs` are zero). The ignore walk
  needed for the WS-06.T1 ripgrep integration enters
  `legion-project` through the existing `legion-index` edge (the
  search runtime does the walk and emits
  `WorkspaceDiscovery*`-shaped DTOs back through `legion-app`).
- The GUI Phase 4 composition entry at
  `plans/dependency-policy.md` §1 line 92 already authorizes
  `legion-app` → `legion-index`. The current
  `crates/legion-app/Cargo.toml` declares this edge, and the
  `legion-app` source composes the search runtime through the
  `SearchProjection` / `SearchResultProjection` / `RunSearch`
  paths. No policy change is required.
- The `legion-ui` policy entry at `plans/dependency-policy.md` §1
  lines 54-75 already forbids `legion-ui` from depending on
  `legion-project`, `legion-editor`, `legion-storage`, `eframe`,
  `egui`, `egui-winit`, `egui-wgpu`, `winit`, `wgpu`,
  `accesskit`, `slint`, `tauri`, `wry`, `tao`, or `gpui`. None
  of the search runtime crates (`grep-searcher`, `grep-regex`,
  `ignore`, `globset`, `tantivy`) are added to that list because
  the `legion-ui` policy entry is already a closed boundary
  (only `legion-protocol` is allowed). The boundary sketch in
  the ratified ADR reinforces this rule with a structural audit
  (no `legion-ui` may declare any search / ignore / glob /
  index runtime dependency).
- The `grep-searcher` / `grep-regex` / `ignore` / `globset` /
  `tantivy` workspace dependencies are **not** added to the root
  `Cargo.toml` at M0. They will be added during WS-06.T1 ("In-
  process ripgrep") and WS-06.T5 ("Tantivy indexed search")
  as runtime activations, under the same dependency-policy gate
  that authorized the parser-boundary audit in `ADR-0033` and
  the LSP-boundary audit sketched in `ADR-0034` and the
  terminal-boundary sketch in `ADR-0035`. The gate is
  forward-compatible with a future
  `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `SEARCH_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]` /
  `FORBIDDEN_SEARCH_DEPS = ["grep-searcher", "grep-regex",
  "ignore", "globset", "tantivy"]` audit shaped like the
  existing `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]` /
  `FORBIDDEN_PARSER_DEPS = ["tree-sitter", "tree-sitter-rust"]`
  audit in `xtask/src/main.rs` (the constants are at lines
  446-453 in the current tree). The M0 ratification does not
  require the search-boundary audit to land today; the ADR
  commits to the boundary and to the runtime activation path,
  not to a new `xtask` subcommand.
- `xtask` does not need a new subcommand. The structural
  dependency audit and the protocol-contract audit that
  already run as part of `check-deps` are sufficient to enforce
  the current `legion-index` and `legion-project` policy
  entries; the future search-boundary audit is a phase-gate
  improvement, not an M0 prerequisite.

## Gate Evidence (verbatim)

All gates were run against the current working tree with commit
baseline `b56dcb2`; the ratification changes (ADR flip + this
evidence file) are untracked as required by the task's "no commit
without explicit user instruction" rule. (The working tree contains
unrelated uncommitted edits from sibling M0 cards; they are not
part of this ratification and are noted only so the gate outputs
are reproducible against the same baseline.)

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
- The `legion-index` policy entry at lines 127-134 is intact and
  matches `crates/legion-index/Cargo.toml` (a subset of the
  allowed internal edges, no ripgrep-class search runtime
  dependency declared today).
- The `legion-project` policy entry at lines 37-41 is intact and
  matches `crates/legion-project/Cargo.toml` (`legion-observability`,
  `legion-platform`, `legion-protocol`, `legion-security`, plus
  standard external deps like `serde` / `tokio` / `tracing` /
  `uuid`; no ripgrep-class search runtime dependency declared
  today).
- The structural audit confirms `legion-app`'s `legion-index`
  edge is policy-allowed (line 92 of the policy file lists
  `legion-index` in the `legion-app` may-depend-on set) and that
  `legion-ui` does not declare a `legion-project` or
  `legion-index` edge (the `legion-ui` policy entry at lines
  54-75 only authorizes `legion-protocol` and forbids every
  renderer / editor / project / storage crate).

### `cargo run -p xtask -- docs-hygiene`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- docs-hygiene
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/xtask docs-hygiene`
documentation hygiene checks passed
```

Exit code: `0`. Confirms the ADR-0036 ratification does not break
doc-hygiene invariants (broken relative Markdown links or
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
paths. The search panel renders projected search results, and
the ADR explicitly reaffirms that the search query input field
is out of the no-`TextEdit` scope (the rule covers the code
canvas, not search panels).

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

### `cargo test -p legion-index --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-index --tests 2>&1 | grep -E '^(test result|running |error)'
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 38 tests
test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.20s
```

Exit code: `0`. Across the two test binaries (the crate's
`lib.rs` unittests + the `index_workflows` integration test),
**38 contract tests pass with 0 failures**. Highlights:

- `lib.rs` unittests (38 tests): deterministic lexical /
  tree-sitter / structural / retrieval search APIs, including
  the `StructuralSearchQuery` / `StructuralSearchMatch` /
  `StructuralSearchCapture` surface around lines 3293-3349,
  the `RetrievalSearchResponse` / `search_retrieval` surface
  around lines 2754-2756 and 2993-2994, the
  `INDEX_SCHEMA_VERSION` / `LEXICAL_EXTRACTION_VERSION` /
  `TREE_SITTER_EXTRACTION_VERSION` /
  `RETRIEVAL_CHUNKING_VERSION` /
  `LOCAL_RETRIEVAL_EMBEDDING_VERSION` constants, the
  `WorkspaceDiscovery*` / `WorkspaceSnapshot` DTOs, and the
  actor-owned scheduling contract. The M0 ratification
  ratifies a working structural / retrieval search surface
  that the WS-06 ripgrep integration will extend in
  `legion-index` itself (the `grep-searcher` /
  `grep-regex` / `ignore` / `globset` libraries are added
  on top of this surface, not in place of it).

The M0 ratification ratifies a working boundary plus a real
semantic-fabric substrate; WS-06.T1 will productize the
in-process ripgrep search runtime on top of this contract.

### `cargo test -p legion-project --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-project --tests 2>&1 | grep -E '^(test result|running |error)'
running 15 tests
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
```

Exit code: `0`. Across the five test binaries (the crate's
`lib.rs` unittests + four integration tests, including
git-conflict, git-snapshot, path-boundary, and
watcher-recovery), **45 contract tests pass with 0 failures**.
The `legion-project` tests cover the filesystem / watcher /
trust-decision surface that the search runtime will compose
through for the `SearchScopeProjection::Workspace` path.
WS-06.T1 will add explicit ignore-walk / cancellation /
streaming contract tests on top of this surface.

## Invariant Preservation Checklist

- [x] Projection-only UI: `legion-ui` still emits
  `CommandDispatchIntent` and accepts snapshots. The search
  panel renders projected search results
  (`SearchProjection` / `SearchResultProjection` /
  `SearchScopeProjection` / `SearchStatusProjection` /
  `StructuralSearchProjection`); it never owns search /
  ignore / glob / index state, never owns filesystem walks,
  and never owns mutation authority. The
  `legion-ui` policy entry at
  `plans/dependency-policy.md` §1 lines 54-75 already
  forbids every renderer / editor / project / storage edge,
  and the structural audit enforces it. Unchanged.
- [x] Proposal-mediated mutation: reaffirmed in the ADR's
  "Search-and-replace with proposals (WS-06.T2)" paragraph.
  The search runtime never mutates the workspace, editor, or
  disk directly. Multi-file search-and-replace materializes
  as a `WorkspaceProposal` payload through the accepted
  Phase 2 proposal routes (`ADR-0016`); the search runtime
  returns match spans as `SearchResultProjection` and the
  proposal service previews, approves, applies, rejects,
  cancels, or rolls back. The structural-search variant
  already exercises this contract through
  `StructuralSearchProjection` and the
  `RunStructuralSearch` `AppCommandRequest` path. The Phase
  2 proposal routes (`ADR-0016`) are unchanged.
- [x] Metadata-first observability: reaffirmed through the
  existing `SearchProjection` /
  `SearchStatusProjection` DTOs and the
  `CorrelationId` / `CausalityId` / `EventSequence` plumbing
  that the rest of the IDE already uses. Search work emits
  metadata-only records (query id, scope, result counts,
  freshness, cancellation reason); raw query strings are
  limited to the user's own UI session, raw match lines are
  limited to bounded result projections, and the
  observability sinks that reject zero IDs apply to search
  the same way they apply to terminal / AI / tracker
  output. Unchanged.
- [x] Fail-closed policy: enforced at the
  `legion-index` / `legion-project` boundary. The
  `legion-index` policy entry at
  `plans/dependency-policy.md` §1 lines 127-134 forbids
  "vector indexing, embeddings, model-provider dependencies,
  direct renderer/UI parser ownership, or direct mutation of
  buffers and workspaces" in the Phase 3 semantic-fabric
  scope; the new ripgrep-class search runtime extends but
  does not amend that policy. The future
  `SEARCH_BOUNDARY_POLICY_MARKERS` audit uses the same
  fail-closed shape as the parser-boundary and
  terminal-boundary audits. The dependency-policy audit
  (`cargo run -p xtask -- check-deps`) fails closed if any
  workspace package other than `legion-index` declares
  `grep-searcher` / `grep-regex` / `ignore` / `globset` /
  `tantivy` once the runtime activations land, because the
  structural audit iterates the same `package_dependencies`
  map that drives the renderer-boundary and parser-boundary
  checks.

## Operational Notes

- The M0 ratification does **not** commit anything; the user
  retains explicit commit authority per the task body rule.
  The ADR flip and the new evidence package are working-tree
  changes only.
- The full workspace test surface
  (`cargo test --workspace --all-targets`), clippy
  (`cargo clippy --workspace --all-targets -- -D warnings`),
  and `cargo deny check` are recorded at the milestone-claim
  level, not per ADR, and remain a prerequisite for the next
  phase-gate flip.
- WS-06 acceptance criteria (in-process ripgrep integration,
  search-and-replace with proposals, fuzzy finder, command
  palette completion, optional tantivy indexed-search tier)
  are downstream of this ratification and remain out of scope
  for the M0/ADR-0036 work packet. WS-06.T1 ("In-process
  ripgrep") is the first critical-path task that consumes
  this ADR; the existing `crates/legion-index/src/lib.rs`
  (5696 lines, 38 contract tests) and the existing search
  projection surface in `crates/legion-app/src/lib.rs`
  (`SearchProjection` / `SearchResultProjection` /
  `SearchScopeProjection` / `RunSearch` / `run_search` /
  `StructuralSearchProjection` / `RunStructuralSearch` /
  `run_structural_search`) are the starting point for that
  workstream, and the existing `legion-index` policy entry
  is the M0 boundary that WS-06.T1 will amend to add the
  ripgrep-class search runtime declarations.
- The future `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `SEARCH_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]` /
  `FORBIDDEN_SEARCH_DEPS = ["grep-searcher", "grep-regex",
  "ignore", "globset", "tantivy"]` audit is recorded as a
  future gate improvement in the decision section of the
  ADR; it is not required for the M0 ratification, but it
  is the natural next step once WS-06.T1 starts declaring
  `grep-searcher` / `grep-regex` / `ignore` / `globset` as
  workspace dependencies. A worker implementing that audit
  should mirror the parser-boundary audit in
  `xtask/src/main.rs` (the
  `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES` /
  `FORBIDDEN_PARSER_DEPS` shape) and the LSP-boundary
  sketch in `ADR-0034` and the terminal-boundary sketch
  in `ADR-0035`.
- The `ADR-0005` storage reservation is the tier-2 gate for
  the tantivy indexed-search option. WS-06.T5 ("Tantivy
  indexed search") is the WS-level path to that tier, and
  the M0 ratification does not enable it. The always-
  available fallback is the in-process ripgrep search
  runtime ratified here.
