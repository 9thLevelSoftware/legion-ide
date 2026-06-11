# ADR-0036: Search and Index Stack

## Status

Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.

This ADR ratifies the Production Master Plan v0.1 §6 recommendation verbatim
(option (a), in-process `grep-searcher` / `grep-regex` / `ignore` / `globset`
ripgrep-class search runtime, with `tantivy` reserved as an optional later
indexed-search tier under `ADR-0005`'s storage reservations), and records the
resulting crate boundary: search/ignore/glob/index runtime is owned by
`legion-index` over `legion-platform` filesystem/process primitives and
consumed by `legion-app` through projection DTOs, the policy-gated workspace
search path, and the proposal-mediated multi-file replace route.

## Context

Legion already has search projection DTOs wired end-to-end in `legion-app`
(`SearchProjection`, `SearchResultProjection`, `SearchScopeProjection`,
`SearchStatusProjection` / `SearchStatusKindProjection`,
`TerminalSearchProjection`, the `RunSearch` / `RunStructuralSearch`
`AppCommandRequest` variants, and the `run_search` / `run_structural_search`
composition entry points — see `crates/legion-app/src/lib.rs` around
lines 130-180, 10046-10051, 10436-10444, 12400-12420, and 15820-15920).
The projection surface is the policy boundary; the underlying ripgrep-class
search runtime is not yet integrated — `grep-searcher`, `grep-regex`,
`ignore`, `globset`, and `tantivy` are not present in `Cargo.lock` and no
workspace package declares them today.

`legion-index` (`crates/legion-index/src/lib.rs`, 5696 lines) is the active
actor-owned semantic-fabric crate. It already implements the deterministic
lexical / tree-sitter / structural / retrieval search surfaces that the
master plan §6 "Search & index stack" row sits next to (the
`StructuralSearchQuery` / `StructuralSearchMatch` /
`StructuralSearchCapture` API around lines 3293-3349, the
`RetrievalSearchResponse` / `search_retrieval` API around lines 2754-2756
and 2993-2994, and the `INDEX_SCHEMA_VERSION`,
`LEXICAL_EXTRACTION_VERSION`, `TREE_SITTER_EXTRACTION_VERSION`,
`RETRIEVAL_CHUNKING_VERSION`, `LOCAL_RETRIEVAL_EMBEDDING_VERSION`,
`LOCAL_RETRIEVAL_EMBEDDING_DIMENSIONS` constants that the search contracts
reference). `legion-index` currently depends on `legion-protocol`,
`legion-text`, `thiserror`, `tree-sitter`, `tree-sitter-rust`, and `uuid`
(per `crates/legion-index/Cargo.toml`) — a strict subset of the
`legion-index` policy entry at `plans/dependency-policy.md` §1 lines
127-134, which already authorizes `legion-protocol`, `legion-storage`,
`legion-text`, `tree-sitter`, and `tree-sitter-rust` and explicitly
forbids "vector indexing, embeddings, model-provider dependencies, direct
renderer/UI parser ownership, or direct mutation of buffers and workspaces"
in the Phase 3 semantic-fabric scope.

`legion-project` (`crates/legion-project/src/lib.rs`, 6802 lines) owns the
workspace discovery, watcher, and trust-decision surface, and its policy
entry at `plans/dependency-policy.md` §1 lines 37-41 currently authorizes
`legion-observability`, `legion-platform`, `legion-protocol`, and
`legion-security`. `legion-project` does not yet contain any ripgrep-class
search code (`grep` / `ignore::` / `globset` / `tantivy` references in
`crates/legion-project/src/lib.rs` are zero). WS-06 ("Search, Navigation
& Command Surface") depends on this ADR; WS-10 ("Semantic Retrieval
& Indexing") and the optional tantivy tier depend on `ADR-0037`.

Four invariants from the master plan §2.2 constrain the search stack:

- **App-composed and capability-gated** — workspace search is a security
  broker decision. Future capability names for search will be reserved
  alongside the existing `terminal.*`, `remote.transport.*`,
  `telemetry.*`, `retention.*`, and `storage.migration.*` reservation set
  in `plans/dependency-policy.md` §1; the search crates may not bypass
  the broker, may not mutate workspace/editor/disk outside the
  proposal-mediated route, and may not persist raw source bodies or
  full-text transcripts (only the metadata already required for
  retrieval records, chunk references, and audit).
- **Proposal-mediated mutation** — multi-file search-and-replace
  (WS-06.T2) materializes as a `WorkspaceProposal` payload through the
  accepted Phase 2 proposal routes (`ADR-0016`). The search runtime
  itself never applies edits to buffers or disk; it returns
  `SearchProjection` / `SearchResultProjection` / match spans and the
  proposal service previews, approves, applies, rejects, cancels, or
  rolls back. This mirrors the "search and replace with proposals" line
  in the master plan §7 WS-06.T2.
- **Projection-only UI boundary** — `legion-ui` consumes search
  projections (`SearchProjection`, `PaletteProjection`,
  `TerminalSearchProjection`, `StructuralSearchProjection`) and emits
  `CommandDispatchIntent` only. UI never owns search/ignore/glob/index
  state, never owns filesystem walks, and never owns mutation
  authority. The `legion-ui` policy entry at
  `plans/dependency-policy.md` §1 lines 54-75 already forbids
  `legion-ui` from depending on `legion-project` or any renderer
  crate, and the structural dependency audit enforces it.
- **Metadata-first observability** — search work emits metadata-only
  records (query id, scope, result counts, freshness, cancellation
  reason, `CorrelationId` / `CausalityId` / `EventSequence`); raw
  query strings are limited to the user's own UI session, raw
  match lines are limited to bounded result projections, and the
  observability sinks that reject zero IDs apply to search the
  same way they apply to terminal/AI/tracker output.

The plan compared three options: (a) `grep-searcher` / `grep-regex` /
`ignore` / `globset` in-process for literal/regex/case/word/glob modes
plus an optional `tantivy` indexed-search tier behind `ADR-0005`'s
storage reservations, (b) subprocess ripgrep, and (c) a custom hand-rolled
search engine. Option (a) reuses BurntSushi's production ripgrep-class
library crates (the same crates that power the `rg` binary itself, kept
as a library so search stays in-process and policy-inspectable, with
file-type filtering, gitignore awareness, hidden-file handling, and
binary-file detection all provided by `ignore`), avoids the
subprocess-spawn / stdio-parse / cancellation-kill-escalation surface
that option (b) would re-introduce, and matches how Zed / Helix /
lapce wire their search stack (in-process ripgrep library, no
subprocess). Option (b) would conflict with the
"reuse audited primitives" rule in `ADR-0026`'s spirit, would re-open
the policy/redaction audit surface for subprocess output, and would
not give Legion any way to cancel searches cleanly without kill
escalation. Option (c) would mean re-implementing ripgrep, which the
plan explicitly rejects as a category mistake.

## Decision

Legion will use the in-process ripgrep-class search runtime as the
primary search surface, with an optional `tantivy` indexed-search tier
reserved for large workspaces and gated by `ADR-0005`'s storage
reservations. The search/ignore/glob/index runtime is owned by
`legion-index` over `legion-platform` filesystem primitives, consumed
by `legion-app` through projection DTOs, and never owned by `legion-ui`,
`legion-editor`, or `legion-desktop`.

- **Search runtime.** `legion-index` owns the in-process ripgrep-class
  search runtime. The runtime uses the `grep-searcher` library crate
  (BurntSushi's production ripgrep factoring) for streaming search,
  `grep-regex` for the regex matcher, `ignore` for `.gitignore` /
  `.ignore` / `.rgignore`-aware directory walks with file-type
  classification, and `globset` for include/exclude glob patterns. The
  result-streaming shape (bounded batches, incremental emission, fresh
  cancellation) follows the same streaming pattern `legion-index`
  already uses for its `StructuralSearchQuery` /
  `RetrievalSearchResponse` APIs. The search runtime is a library
  integration, not a subprocess; policy-inspectable in the sense that
  every match is a typed `SearchResultProjection` that the policy
  layer can re-emit through the proposal service.
- **Ignore / file-type walk.** `ignore` provides the workspace walk
  (`.gitignore` / `.ignore` / global ignore / hidden-file policy /
  binary detection) and `globset` provides include/exclude glob
  matching. Both are already canonical in the Rust ripgrep ecosystem
  and ship in the same family as `grep-searcher`; using them together
  is the standard composition. The ignore-walk and the glob matchers
  are integrated behind the existing `WorkspaceDiscovery*` and
  `WorkspaceSnapshot` DTOs in `legion-protocol` and the existing
  `legion-project` policy entry at
  `plans/dependency-policy.md` §1 lines 37-41, so the walk stays
  trust-aware and policy-gated; untrusted workspaces still walk, but
  results carry the trust label and the proposal service refuses to
  apply edits to untrusted targets.
- **Indexed search (optional tier).** `tantivy` is reserved for the
  WS-06.T5 indexed-search tier for >100K-file workspaces where live
  ripgrep degrades. Tantivy activation is gated by `ADR-0005`'s
  storage reservations, requires a fresh policy entry addition when
  the tier is productized, and is explicitly optional — Legion ships
  without it first and turns it on only after large-fixture evidence
  shows a crossover benefit. This matches the master plan §6
  "Tantivy activates under ADR-0005's storage reservations" line and
  the WS-06.T5 acceptance criteria.
- **Crate boundary.** `legion-index` is the only workspace crate
  authorized to declare `grep-searcher`, `grep-regex`, `ignore`,
  `globset`, or `tantivy` (or any successor search / ignore / glob /
  index crate) once those are added. `legion-app` composes search
  results through the existing `SearchProjection` /
  `SearchResultProjection` / `SearchScopeProjection` /
  `StructuralSearchProjection` DTOs and the `RunSearch` /
  `RunStructuralSearch` `AppCommandRequest` paths. `legion-project`
  may use the `ignore` walker through `legion-index` (and may
  declare `grep-searcher` / `grep-regex` / `globset` / `tantivy`
  only if the policy entry is amended at that time, which is not
  part of the M0 ratification). `legion-ui` consumes search
  projections and emits `CommandDispatchIntent` only. `legion-editor`
  may not declare any search / ignore / glob / index runtime
  dependency. `legion-desktop` may not declare any search / ignore /
  glob / index runtime dependency. This boundary mirrors the
  parser-boundary audit in `ADR-0033`, the LSP-boundary sketch in
  `ADR-0034`, and the terminal-boundary sketch in `ADR-0035`, and is
  enforced by the same `cargo run -p xtask -- check-deps`
  policy-text + package-dependency audit.
- **Search-and-replace with proposals (WS-06.T2).** Multi-file
  search-and-replace materializes as a `WorkspaceProposal` payload
  through the accepted Phase 2 proposal routes (`ADR-0016`). The
  search runtime returns match spans (file identity, line/column
  range, capture groups) as `SearchResultProjection`; the proposal
  service generates a preview per match, lets the user accept /
  reject individual matches, applies the accepted set as one
  reversible proposal, and supports partial selection. The
  search-and-replace path is not a direct edit. This mirrors the
  master plan §7 WS-06.T2 acceptance line and the projection-only /
  proposal-mediated invariants.
- **Fuzzy finder (WS-06.T3).** The fuzzy file / symbol / recent-buffer
  finder is a separate surface from ripgrep search and uses a
  nucleo-style scorer (or any equivalent deterministic, in-process
  matcher) on top of the structural and lexical maps `legion-index`
  already maintains (`StructuralSearchQuery` plus the file / symbol
  maps fed by the Phase 3 semantic fabric). The fuzzy matcher is a
  library integration in `legion-index`, not a subprocess and not a
  re-implementation; it is governed by the same crate boundary as
  ripgrep search above.
- **Command palette (WS-06.T4).** The command palette covers every
  registered `CommandDispatchIntent` and is already projection-only
  on top of `PaletteProjection` / `PaletteResult` / `PaletteResultKind`
  in `legion-app`. The palette stays keyboard-first and never owns
  search/ignore/glob/index state; the match score for the palette
  uses the same in-process matcher the fuzzy finder uses.
- **Indexing tier (WS-10).** The semantic-retrieval / vector /
  agentic-search workstream is governed by `ADR-0037`. The
  ripgrep-class search runtime ratified here is the *primary*
  product surface for project search; the tantivy tier is
  secondary; the vector / agentic tier is a future option. The
  three tiers are independent in the dependency policy and
  activate on separate gates (ADR-0036, ADR-0036+tier-2, ADR-0037).
- **Crate boundary audit (future gate).** A future
  `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `SEARCH_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]` /
  `FORBIDDEN_SEARCH_DEPS = ["grep-searcher", "grep-regex", "ignore",
  "globset", "tantivy"]` audit is sketched in the decision for the
  next phase gate, shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]` /
  `FORBIDDEN_PARSER_DEPS = ["tree-sitter", "tree-sitter-rust"]` audit
  in `xtask/src/main.rs` (the constants are at lines 446-453 in the
  current tree). The M0 ratification does not require the
  search-boundary audit to land today; the audit is a phase-gate
  improvement that becomes useful the moment a workspace package
  actually declares one of the forbidden search / ignore / glob /
  index crates. Today, no package declares any of them, so the
  audit is a forward-compatibility gate, not a regression guard.
- **Compatibility with `ADR-0005` storage reservations.** The tantivy
  tier activates only after the existing `ADR-0005` storage backend
  decisions are accepted, only on the storage surface the `legion-storage`
  policy entry at `plans/dependency-policy.md` §1 lines 32-35
  authorizes, and only through the accepted `legion-storage` metadata
  surface. M0 does not flip the tantivy tier; the WS-06.T5 product
  workstream is the path to that flip, with the in-process ripgrep
  search runtime as the always-available fallback.

## Consequences

- **Positive:** reuses BurntSushi's production ripgrep-class library
  crates (the same family that powers the `rg` binary), keeps
  search in-process and policy-inspectable, and avoids
  subprocess-spawn / stdio-parse / cancellation-kill-escalation
  surface. The grep-searcher / ignore / globset combination is
  what every shipping Rust editor wires for project search (Zed,
  Helix, lapce) and matches the "reuse audited primitives" rule
  that has been the consistent pattern across `ADR-0026`,
  `ADR-0033`, `ADR-0034`, and `ADR-0035`.
- **Positive:** the M0 ratification ratifies a working projection
  boundary. The `SearchProjection` / `SearchResultProjection` /
  `SearchScopeProjection` / `StructuralSearchProjection` /
  `TerminalSearchProjection` DTOs, the `RunSearch` /
  `RunStructuralSearch` `AppCommandRequest` variants, and the
  `run_search` / `run_structural_search` composition entry points
  are all live in `legion-app` today. The deterministic
  `StructuralSearchQuery` / `RetrievalSearchResponse` /
  `search_retrieval` APIs in `legion-index` are live, exercised
  by 38 `legion-index` contract tests, and the no-`TextEdit` /
  proposal-mediated / projection-only / metadata-first invariants
  apply to search through the same `legion-app` plumbing that
  already enforces them for the rest of the IDE.
- **Positive:** the WS-06 workstreams have a real starting point
  for ripgrep-class search integration, fuzzy finder, command
  palette completion, and tantivy tier-2. The semantic-fabric
  work in `legion-index` (Phase 3, `ADR-0017`, `ADR-0033`) and
  the search projection work in `legion-app` (this ratification)
  share a single crate boundary, so the product surface stays
  one IDE and not two search experiences.
- **Negative:** introducing `grep-searcher` / `grep-regex` / `ignore`
  / `globset` is a WS-06 runtime activation, not an M0 prerequisite.
  The M0 ratification ratifies the boundary and the stack choice;
  the WS-06.T1 ("In-process ripgrep") task is the workstream that
  declares the dependencies, extends the `legion-index` policy
  entry, adds the search-boundary audit, and ships the integration
  test that benchmarks Legion-repo-wide search < 150ms warm.
- **Negative:** in-process search increases dependency and
  integration surface; the dependency surface stays in
  `legion-index` and the integration surface stays in
  `legion-app` / `legion-project`, so neither `legion-ui` nor
  `legion-editor` grows a search / ignore / glob / index edge.
- **Negative:** tantivy is a larger dependency than
  grep-searcher / ignore / globset. The optional tier is
  explicitly reserved; the M0 ratification does not enable it.
- **Mitigation:** the `legion-index` policy entry is the only
  edge that needs the new `grep-searcher` / `grep-regex` /
  `ignore` / `globset` declarations, and the future
  `SEARCH_BOUNDARY_POLICY_MARKERS` audit is the structural guard
  that prevents any other crate from declaring them. The
  structural dependency audit that already runs as part of
  `cargo run -p xtask -- check-deps` is the M0 test surface; the
  WS-06.T1 acceptance criteria are the warm-latency /
  cancellation / streaming / gitignore / glob test surface.

## Verification

- `cargo run -p xtask -- check-deps` (dependency direction +
  structural audit, with the `legion-index` and `legion-project`
  policy entries verified against `plans/dependency-policy.md` §1
  and the search-boundary sketch above)
- `cargo run -p xtask -- docs-hygiene` (broken relative Markdown
  links and the unallowlisted stale Legion-rename marker)
- `cargo run -p xtask -- no-egui-textedit` (companion gate,
  unchanged from `ADR-0032`; the search panel renders projected
  search results, not an `egui::TextEdit` for the search query
  itself; the query input is a single-line egui widget that is
  out of the no-`TextEdit` scope)
- `cargo fmt --all --check`
- `cargo test -p legion-index --tests` (deterministic lexical /
  structural / retrieval search APIs, 38 contract tests across
  the lib unittests + the `index_workflows` integration test,
  covering the existing `StructuralSearchQuery` /
  `RetrievalSearchResponse` / `search_retrieval` surface that the
  ripgrep search runtime will extend during WS-06.T1)
- `cargo test -p legion-project --tests` (workspace discovery /
  watcher / trust surface that backs the search ignore-walk)
- `cargo test -p legion-app --tests` (search / structural-search
  / fuzzy-finder / command-palette projection surface, including
  the `run_search` / `run_structural_search` composition entry
  points and the `RunSearch` / `RunStructuralSearch`
  `AppCommandRequest` paths)
- WS-06 evidence under `plans/evidence/production/m1/` once the
  in-process ripgrep integration is product-validated (out of
  scope for this M0 ratification); WS-06.T5 evidence for the
  tantivy tier lands under the same directory once
  `ADR-0005`'s storage reservations are accepted
