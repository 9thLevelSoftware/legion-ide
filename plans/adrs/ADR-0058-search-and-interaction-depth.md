# ADR-0058: Concentrate worktree search and app interaction workflows

## Status

Accepted scope, 2026-10-08. Local implementation evidence is recorded in
`../evidence/production/search-and-interaction-depth-2026-10-08.md`.

## Decision

Grep and glob share worktree traversal rules inside `legion-agent`, preserving
their distinct matching and result-limit semantics. The shared implementation
owns hidden-directory filtering, forbidden-path filtering, non-following entry
classification, recursion, and error propagation. Existing execution policy,
audit timing, and tool feedback remain in force. Real temporary worktrees are
the test dependency; no speculative filesystem adapter is introduced.

`legion-app` coordinates completion and hover scheduling from accepted actions
and authoritative editor outcomes. Completion retains its 50 ms delay and hover
its 200 ms delay. `legion-desktop` retains palette input ownership, frame timing,
and popup presentation. Semantic scheduling no longer depends on desktop callers
choosing pre-action versus post-action coordinates or translating elapsed timers
back into app intents. Existing LSP transport, document synchronization, and
proposal authority remain with their established owners.

Explorer file activation is an app-owned workflow: open successfully, then
reveal and update explorer selection. A failed open must not select the failed
target. The desktop adapter translates projected rows; directory activation
continues to toggle expansion rather than opening a text buffer.

## Rationale and constraints

These changes concentrate repeated traversal rules and caller ordering
obligations rather than merely moving functions into new files. They extend
the existing ownership decisions in ADR-0030 and ADR-0034 without reopening the
Git, LSP write-lifecycle, or Vim work recorded in ADR-0057. No new crate,
dependency edge, provider capability, or workspace mutation authority is added.

## Verification gate

- Exercise both search tools through their existing interface, including
  forbidden descendants, alias roots, links/cycles, limits, and I/O failures.
- Verify app scheduling against resulting editor positions, unchanged delays,
  no-op/failure behavior, and tab invalidation.
- Preserve desktop input, completion, hover, and explorer activation contracts,
  including palette ownership and failed-open selection.
- Run formatting, affected-crate lint, dependency-policy, and documentation gates.

This gate establishes local refactor evidence only. Cross-platform CI, packaged
native-input acceptance under ADR-0056, performance measurements, and release
qualification remain separate; no product-readiness row is promoted.
