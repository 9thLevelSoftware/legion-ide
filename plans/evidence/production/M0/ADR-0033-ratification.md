# M0 — ADR-0033 (Syntax/Parse Engine) Ratification Evidence

Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
ADR: [`plans/adrs/ADR-0033-syntax-parse-engine.md`](../../../adrs/ADR-0033-syntax-parse-engine.md)
Date: 2026-06-10
Gate: `cargo run -p xtask -- check-deps` (parser-boundary enforcement:
`PARSER_BOUNDARY_POLICY_MARKERS`, `PARSER_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]`,
`FORBIDDEN_PARSER_DEPS = ["tree-sitter", "tree-sitter-rust"]`)
Acceptance target: master-plan §6 row "ADR-0033 | Syntax/parse engine" → option (a) ratified in-repo, with the parse engine kept inside `legion-index` and out of every other workspace crate.

## Decision Recorded

- Status flipped from `Draft` to `Accepted` in `plans/adrs/ADR-0033-syntax-parse-engine.md`.
- Decision text matches Production Master Plan v0.1 §6 recommendation verbatim (tree-sitter; syntect remains allowed in `legion-app` for read-only Phase 2 fallback enrichment and is to be removed later; bundled native grammars for the WS-02 flagship set; wasm grammar distribution is the future Phase 5 plugin-channel path).
- No amendments were required to the master-plan recommendation. The ADR adds three confirmations consistent with the plan and with current code/tests:
  1. The parser-boundary gate inside `xtask check-deps` is the M0/WS02 enforcement mechanism (decision + verification sections).
  2. `legion-index` already pins the bundled-grammar extraction contract via `TREE_SITTER_EXTRACTION_VERSION = "legion-index-tree-sitter-v1"` and the `SemanticGrammarVersion` DTO used in `crates/legion-index/tests/index_workflows.rs` (consequences section).
  3. The renderer/UI boundary is preserved: `legion-ui` keeps consuming projections/tokens only; tree-sitter and grammar crates never reach `legion-ui`, `legion-desktop`, `legion-editor`, or `legion-app` (decision section).

## Crate / Dependency Boundary Impact

- No new internal crate edges are introduced by this ADR.
- `legion-index` remains the only workspace crate authorized to declare the
  `tree-sitter` and `tree-sitter-rust` external dependencies, per the
  `legion-index` policy entry in `plans/dependency-policy.md` §1 ("Phase 3
  semantic fabric activation for `crates/legion-index/Cargo.toml` is limited
  to the three internal dependencies listed above plus parser-only
  tree-sitter runtime and bundled Rust grammar crates for WS02 syntax
  activation").
- `legion-index/Cargo.toml` already declares both crates as workspace
  dependencies (`tree-sitter = { workspace = true }`,
  `tree-sitter-rust = { workspace = true }`). The parser-boundary gate
  prevents any other crate from adding the same dependencies; if a future
  refactor adds them elsewhere, `cargo run -p xtask -- check-deps` fails
  with a `workspace package <name> must not declare parser/runtime
  dependencies outside legion-index` violation.
- `xtask` does not need a new subcommand. The parser-boundary audit already
  runs as part of `check-deps` (it iterates the same `package_dependencies`
  map and the same `policy_text` that drive the renderer-boundary and
  structural dependency checks, see `xtask/src/main.rs:659-870` and the
  `validate_parser_dependency_gate` helper at `xtask/src/main.rs:1045-1091`).
- `legion-app` and `legion-desktop` remain projection-only: they consume
  `ViewportSemanticTokenOverlay` values produced by `legion-index` and map
  them to theme colors. They do not gain a tree-sitter dependency.

## Gate Evidence (verbatim)

All gates were run against the current working tree with commit baseline
`b56dcb2`; the ratification changes (ADR flip + this evidence file) are
untracked as required by the task's "no commit without explicit user
instruction" rule.

### `cargo run -p xtask -- check-deps`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- check-deps
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.34s
     Running `target/debug/xtask check-deps`
dependency policy checks passed
```

Exit code: `0`. The parser-boundary audit, the renderer-boundary audit, the
structural dependency audit, the protocol-contract audit, and the phase 3/4/5/
6/7/8/13 acceptance governance audits all pass against the current tree. In
particular, `validate_parser_dependency_gate` confirms:

- `plans/dependency-policy.md` still contains every
  `PARSER_BOUNDARY_POLICY_MARKERS` string.
- The only workspace package that declares `tree-sitter` or `tree-sitter-rust`
  is `legion-index`; the loop over `packages` skips `legion-index` via
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES` and finds zero forbidden declarations
  elsewhere.

### `cargo run -p xtask -- docs-hygiene`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- docs-hygiene
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/xtask docs-hygiene`
documentation hygiene checks passed
```

Exit code: `0`. Confirms the renamed ADR and the new M0 evidence package do
not break doc-hygiene invariants (broken relative Markdown links or
unallowlisted stale Legion-rename markers).

### `cargo run -p xtask -- no-egui-textedit`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- no-egui-textedit
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/xtask no-egui-textedit`
no-egui-textedit checks passed
```

Exit code: `0`. Companion gate (ADR-0032) unchanged; this ratification did
not touch the painter module or its scanned paths.

### `cargo fmt --all --check`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo fmt --all --check
$ echo $?
0
```

Exit code: `0`. No formatting drift introduced by the ratification changes.

### `cargo check -p xtask`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo check -p xtask
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.90s
```

Exit code: `0`. Confirms the `xtask` crate still builds cleanly with the
parser-boundary markers and audit logic unchanged.

## Invariant Preservation Checklist

- [x] Projection-only UI: `legion-ui` still emits `CommandDispatchIntent` and
  accepts snapshots; tree-sitter and grammar crates stay out of `legion-ui`
  per the dependency policy and the parser-boundary gate. Unchanged.
- [x] Proposal-mediated mutation: unaffected. This ADR only governs the
  syntax/structural parse engine, not the proposal, save, or workspace
  mutation paths.
- [x] Metadata-first observability: unaffected. Tree-sitter extraction in
  `legion-index` is bounded source-text only and runs behind the same
  observability sinks that reject zero `CorrelationId` / nil `CausalityId` /
  zero `EventSequence` as the rest of the workspace.
- [x] Fail-closed policy: enforced by the parser-boundary gate itself — any
  future re-introduction of `tree-sitter` / `tree-sitter-rust` outside
  `legion-index` fails the gate at the same place the saver rejects a
  stale/conflict/denial outcome. The gate reads the live policy text, so the
  next dependency-policy edit that drops a parser-boundary marker also
  fails closed.

## Operational Notes

- The M0 ratification does **not** commit anything; the user retains
  explicit commit authority per the task body rule. The ADR flip and the new
  evidence package are working-tree changes only.
- The full workspace test surface (`cargo test --workspace --all-targets`),
  clippy (`cargo clippy --workspace --all-targets -- -D warnings`), and
  `cargo deny check` are recorded at the milestone-claim level, not per ADR,
  and remain a prerequisite for the next phase-gate flip.
- WS-02 acceptance criteria (highlighting, folding, symbols, sticky headers,
  AST-aware chunking, repo maps) are downstream of this ratification and
  remain out of scope for the M0/ADR-0033 work packet. WS02.T1
  ("tree-sitter runtime integration") is the first critical-path task that
  consumes this ADR; the existing `crates/legion-index` integration and its
  test coverage at `crates/legion-index/tests/index_workflows.rs` are the
  starting point for that workstream.
