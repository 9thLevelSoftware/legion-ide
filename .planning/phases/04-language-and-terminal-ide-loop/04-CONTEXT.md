# Phase 4: Language and Terminal IDE Loop -- Context

## Workflow Inputs

- Command: `$legion plan 4 --auto-refine`
- Roadmap phase: Phase 4, "Language and Terminal IDE Loop"
- Requirements: R-009, R-013
- Planning mode: auto-refine enabled
- Settings: `.planning/config/settings.json` is absent, so the workflow default of at most three implementation tasks per plan applies.
- Control modes: `.planning/config/control-modes.yaml` is absent, so workflow-common guarded defaults apply.
- GitHub issue creation: skipped because `git remote get-url origin` returned `No such remote 'origin'`.

## Map Freshness Warning

`.planning/CODEBASE.md` was generated for commit `b521ab5...`; the current HEAD is `dba01a4f73c021fbb4faeb8b6d48be0ec8ac4fdc`. Use the map only as orientation. Every implementation plan requires build agents to read live source before editing.

Live source evidence found during planning:

- `devil-desktop` exists and is part of the workspace, so older map statements that no GUI renderer exists are stale.
- `crates/devil-app/Cargo.toml` does not currently depend on `devil-index` or `devil-terminal`.
- `plans/dependency-policy.md` still has the older product-roadmap numbering where Phase 4 activates agent/tracker/memory crates and Phase 8 mentions terminal activation.
- `xtask/src/main.rs` still treats `plans/evidence/phase-4/agentic-ai-architecture-map.md` as the legacy Phase 4 evidence path.
- `crates/devil-protocol/src/lib.rs` already contains LSP DTOs, proposal conversion helpers, and terminal DTOs, but the current UI/app/desktop projections do not expose Phase 4 language or terminal panels.
- `crates/devil-index/src/lib.rs` has a semantic index and query surface suitable for local definition, reference, hover, completion-ranking, outline, and refactoring-preview projections without direct buffer mutation.
- `crates/devil-terminal/src/lib.rs` has a default-disabled runtime with launch, input, resize, bounded output polling, close, kill, and metadata-oriented audit coverage.
- `crates/devil-security/src/lib.rs` denies terminal runtime by default and requires workspace trust/capability/command metadata for launch.
- `crates/devil-ui/src/ui.rs`, `crates/devil-app/src/lib.rs`, and `crates/devil-desktop/src/{bridge,workflow,view}.rs` have no language tooling or terminal workflow intents/projections yet.

## Phase Goal

Add the minimum language-tooling and terminal workflow expected from a local IDE while preserving the established Devil ownership model.

Roadmap success criteria:

- Problems panel, diagnostics, hover, completion, go-to-definition, references, outline, formatting, rename, organize imports, and code-action surfaces are visible.
- Edit-producing language actions become proposal previews before mutation.
- A policy-gated terminal panel supports launch, input, resize, kill, bounded output, search/scrollback status, and denial/error states.
- Terminal and LSP cannot mutate editor buffers or disk directly.
- Failures are visible, cancellable where applicable, and metadata-audited.

## Non-Negotiable Constraints

- `devil-ui` remains projection-only. It may emit `CommandDispatchIntent` values and accept snapshots, but it must not own editor sessions, text, terminal runtime state, or LSP/index services.
- Saves and language edits remain proposal-mediated through app/workspace authority. Formatting, rename, organize imports, and code actions must not mutate buffers or disk directly.
- Terminal launch and PTY interaction are policy-gated. Denials and runtime errors must be visible in projections.
- Terminal output must remain bounded and redacted according to the terminal/runtime contracts. Durable audit records must be metadata-oriented.
- LSP/index work must not block editor input or save workflows.
- Any dependency-policy or `xtask check-deps` change must preserve current phase gates and explain the legacy Phase 4 evidence collision.
- Build agents must stop with `BLOCKED` if implementation requires direct workspace/editor mutation from `devil-index`, `devil-terminal`, `devil-ui`, or `devil-desktop`.

## Plan Structure

Six sequential waves are used because the phase crosses shared ownership boundaries and high-risk files:

1. `04-01`: Governance And Projection Contract Rebaseline
2. `04-02`: App Language Tooling Composition And Proposal Routing
3. `04-03`: Policy-Gated Terminal App Workflow
4. `04-04`: Desktop Language And Terminal Panels
5. `04-05`: Cross-Boundary Safety And Failure Tests
6. `04-06`: Phase 4 Evidence And Acceptance Gate

No same-wave plans edit the same high-risk source files. The sequence deliberately establishes policy/projection contracts before app composition and desktop rendering.

## Auto-Refine Summary

Auto-refine identified five planning risks and the plans below include the corrections:

- Legacy Phase 4 governance in `dependency-policy.md` and `xtask` conflicts with the GUI roadmap Phase 4. Plan 04-01 makes that collision explicit and requires a compatibility-preserving rebaseline before adding app edges.
- LSP DTOs already exist, but desktop/app projections do not. Plan 04-01 creates projection contracts first; Plan 04-02 composes behavior afterward.
- Terminal runtime exists but is default-disabled and policy-gated. Plan 04-03 routes terminal lifecycle through app/security authority and denial projections instead of enabling raw PTY access from the renderer.
- Edit-producing language actions can look like direct editor commands. Plan 04-02 requires proposal previews and regression tests for formatting, rename, organize imports, and code actions.
- The final acceptance criteria require evidence across policy, app, desktop, safety, and full gates. Plan 04-06 owns the archived evidence and state update after all build gates pass.
