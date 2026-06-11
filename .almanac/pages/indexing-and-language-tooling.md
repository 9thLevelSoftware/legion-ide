---
title: Indexing And Language Tooling
summary: "`legion-index` provides semantic and lexical projections that refresh UI language surfaces and generate proposals instead of mutating editor or disk state directly."
topics: [indexing, flows, architecture]
sources:
  - id: index-lib
    type: file
    path: crates/legion-index/src/lib.rs
    note: Defines the indexing and semantic query boundary.
  - id: dependency-policy
    type: file
    path: plans/dependency-policy.md
    note: Restricts `legion-index` to protocol, storage, and text dependencies.
  - id: language-tests
    type: file
    path: crates/legion-app/tests/language_tooling_workflow.rs
    note: Verifies completion, diagnostics, hover, rename, formatting, and proposal-preview behavior.
  - id: app-lib
    type: file
    path: crates/legion-app/src/lib.rs
    note: Shows app composition consuming `legion-index` outputs into UI projections.
status: active
verified: 2026-06-08
---
`[[crates/legion-index/src/lib.rs]]` is the semantic and lexical analysis boundary. The crate docstring describes actor-owned scheduling, repository discovery, lexical symbol maps, parser-cache fallbacks, and pure query DTOs rather than editor-owned mutation logic [@index-lib]. The dependency policy keeps that boundary narrow: `legion-index` may depend only on protocol, storage, and text, not on app, UI, editor, or workspace internals [@dependency-policy].

## What the app does with index output

`legion-app` imports `LexicalIndexer`, `SemanticIndex`, retrieval queries, structural search, and structural rewrite preview helpers, then turns those into `LanguageToolingProjection` and proposal rows instead of handing indexing authority back to UI [@app-lib].

## Proven behavior

`[[crates/legion-app/tests/language_tooling_workflow.rs]]` shows the intended contract:

- completion refreshes language projections without giving UI text ownership [@language-tests]
- formatting creates a workspace-edit proposal and leaves the editor text unchanged until proposal application [@language-tests]
- rename previews do not mutate disk [@language-tests]
- diagnostics can project quick fixes whose follow-up code action becomes a proposal preview rather than an immediate write [@language-tests]
- breadcrumbs and sticky scopes come from index-derived symbols [@language-tests]

The important boundary is that semantic tooling can explain, locate, and preview changes, but durable mutation still routes through the proposal/workspace authority chain in [[workspace-save-workflow]].
