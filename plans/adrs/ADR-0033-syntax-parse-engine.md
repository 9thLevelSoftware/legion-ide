# ADR-0033: Syntax and Parse Engine

## Status

Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.

This ADR ratifies the Production Master Plan v0.1 §6 recommendation verbatim
(option (a), tree-sitter as the syntax/structural parse engine), and records
the parser-boundary gate that keeps tree-sitter runtime and bundled grammars
inside `legion-index` and out of every other workspace crate.

## Context

Legion has token/theme plumbing but no real tokenizer wired into the render
path. WS-02 requires incremental parsing for highlighting, folding, symbols,
sticky headers, AST-aware chunking, and repo maps. WS-10 (semantic retrieval)
and WS-15 (per-language intelligence on the agent harness) both depend on the
same structural substrate, and ADR-0037 (semantic retrieval) explicitly leans
on tree-sitter AST-aware chunking for its `(a)` recommendation.

The plan compares tree-sitter and syntect. Syntect can remain a read-only
fallback in `legion-app` for the GUI Phase 2 viewport-token enrichment path
allowed by `plans/dependency-policy.md` §1, but it does not provide the
structural substrate needed for retrieval, folding, or plugin-delivered
grammars. Tree-sitter gives Legion one engine for highlighting, structural
navigation, folding, AST-aware chunking, and future wasm grammar distribution
through the Phase 5 plugin channel.

## Decision

Legion will use tree-sitter as the syntax and structural parse engine.

- `legion-index` owns the tree-sitter runtime, the bundled `tree-sitter-rust`
  grammar, and the highlight-query → semantic-token mapping that feeds
  `ViewportSemanticTokenOverlay`. The implementation already lives at
  `crates/legion-index/src/lib.rs` (`TREE_SITTER_EXTRACTION_VERSION =
  "legion-index-tree-sitter-v1"`, the highlight-capture → token-kind mapper,
  the bounded source-text contract, and the bundled-grammar capability check).
- M1 work bundles native grammars and highlight queries for the WS-02 flagship
  language set (Rust, TOML, Markdown, JSON, TS/JS, Python, Go, C/C++, YAML,
  Bash). Grammars ship compiled-in for the bundled set and stay there
  through M1; WS02.T4 is the future opt-in path for `grammar-as-wasm`
  distribution through the Phase 5 plugin channel.
- The syntax layer must remain outside `legion-ui`. `legion-ui` consumes
  projections/tokens only; it does not own parser state, parse workers, or
  grammar crates. The renderer paints highlight overlays produced by
  `legion-index` and projected through `legion-app` per the Phase 2
  semantic-token enrichment policy in `plans/dependency-policy.md` §1.
- The existing `xtask check-deps` parser-boundary gate
  (`PARSER_BOUNDARY_POLICY_MARKERS`, `PARSER_DEPENDENCY_ALLOWED_PACKAGES =
  ["legion-index"]`, `FORBIDDEN_PARSER_DEPS = ["tree-sitter", "tree-sitter-rust"]`)
  is the M0/WS02 enforcement mechanism. It fails the build if any workspace
  package other than `legion-index` declares a tree-sitter dependency or if
  the dependency policy stops documenting the parser-boundary markers.

## Consequences

- **Positive:** one engine supports highlighting, structural navigation,
  folding, AST-aware chunking, and future grammar distribution; the same
  substrate feeds both WS-02 syntax intelligence and WS-10/WS-15
  retrieval/agent contexts.
- **Negative:** new external dependencies and a grammar supply-chain policy
  must be recorded before more grammars activate. Tree-sitter grammars are
  per-language crates that will need CI coverage, deterministic extraction
  versions, and a permission story before any `grammar-as-wasm` distribution
  goes live.
- **Mitigation:** the parser-boundary gate keeps tree-sitter from leaking into
  renderer, app, or UI crates. Bundled-grammar activation is a per-language,
  per-extension decision that requires the same ADR/dependency-policy/contract
  test/evidence pattern as the rest of the M0 ratifications. The
  `TREE_SITTER_EXTRACTION_VERSION` constant and `SemanticGrammarVersion`
  contract in `legion-index` are the place to pin grammar versions.

## Verification

- `cargo run -p xtask -- check-deps` (parser-boundary audit, phase gates,
  protocol contract, and phase 3/4/5/6/7/8/13 acceptance governance)
- `cargo run -p xtask -- docs-hygiene` (broken relative Markdown links and
  the unallowlisted stale Legion-rename marker)
- `cargo run -p xtask -- no-egui-textedit` (companion gate, unchanged)
- `cargo fmt --all --check`
- `cargo check -p xtask`
- WS-02 evidence under `plans/evidence/production/m1/` once the render path
  is product-validated (out of scope for this M0 ratification).
