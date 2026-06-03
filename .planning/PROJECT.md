# Legion IDE GUI Productization

## What This Is
Productize the existing Legion IDE Rust workspace into a renderer-backed desktop IDE GUI. The current codebase already has substantial editor, workspace, protocol, proposal, policy, storage, semantic, AI, plugin, collaboration, remote, terminal, telemetry, and retention substrate, but the runnable app is still a CLI shell proof and `legion-ui` is projection-only.

## Core Value
A local-first, control-first desktop IDE where ordinary development workflows are fast and visible, while mutation-capable AI, LSP, terminal, plugin, collaboration, and remote actions remain proposal-mediated, auditable, policy-gated, and privacy-aware.

## Who It's For
- Developers who want a serious local IDE with strong safety around generated edits, terminal execution, saves, plugins, collaboration, remote work, and AI assistance.
- Internal Legion IDE users validating the product on real repositories.
- Later external users who need a daily-driver desktop IDE rather than a CLI proof or architecture substrate.

## Requirements

### Validated
(None yet - ship to validate)

### Active
- **R-001 Baseline reconciliation**: Reconcile current planning truth before treating the GUI track as authoritative, especially the Phase 8 ledger/evidence conflict noted in the exploration.
- **R-002 Renderer decision gate**: Accept a renderer integration ADR before adding a GUI framework dependency. The ADR must compare Rust-native GPU, egui/eframe, Slint, and Tauri/WRY against Windows-first IDE requirements and fallback criteria.
- **R-003 Dependency policy update**: Update `plans/dependency-policy.md` and `xtask` checks before introducing renderer dependencies or a desktop crate.
- **R-004 Desktop adapter boundary**: Add a renderer-backed desktop adapter crate or binary, likely `legion-desktop`, that consumes projections and emits intents without owning editor, workspace, proposal, storage, terminal, AI, plugin, collaboration, or remote authority.
- **R-005 Projection rendering**: Render layout, explorer, active buffer viewport, status, proposal summary, trust/privacy summaries, and degraded/dirty states from `ShellProjectionSnapshot` and related projection DTOs.
- **R-006 Intent bridge**: Route window, menu, keyboard, mouse, command palette, and file-dialog actions into `CommandDispatchIntent` or explicit app-level requests handled by `AppComposition`.
- **R-007 Open/edit/save GUI loop**: Let users launch the GUI, open a workspace, browse files, open/edit files, undo/redo, save, quit, and see saved/rejected/conflict outcomes without CLI fallback.
- **R-008 Daily editing MVP**: Support multi-tab editing, explorer expand/collapse/selection, cursor/selection/scrolling, search in file/workspace, save all, close-dirty prompts, conflict handling, and session restore.
- **R-009 Language and terminal loop**: Expose diagnostics, hover, completion, definitions, references, formatting, rename, code actions, and terminal workflows while keeping mutation-capable outputs proposal-mediated.
- **R-010 Control and trust surfaces**: Build proposal ledger, proposal details, preview/diff, approval checklist, rollback/checkpoint, context manifest, privacy inspector, and permission/risk/cost budget panels.
- **R-011 Assisted AI GUI**: Add local-first/default-deny assisted AI flows that show context, provenance, refusals, redaction, and proposed edits without autonomous apply.
- **R-012 Packaging and platform integration**: Produce a Windows desktop package with menus, file dialogs, clipboard, shortcuts, focus traversal, IME, high-DPI handling, accessibility tree, diagnostics export, and smoke-test scripts.
- **R-013 Performance and reliability evidence**: Archive renderer-backed p50/p95 input-to-paint, frame variance, large-file degraded mode, non-blocking background work, crash-safe session restore, and GUI save-conflict evidence.
- **R-014 Local IDE beta**: Reach a beta that can be daily-driven on a real Rust repository for open, browse, edit, search, save, terminal, language features, proposal review, and privacy-safe diagnostics.
- **R-015 Advanced GUI GA**: Expose plugin management, collaboration, remote workspace, delegated task command center, and cross-platform release workflows only after their policy/proposal/privacy gates are preserved.

### Out of Scope
- Rebuilding editor, workspace, proposal, semantic, AI, plugin, collaboration, remote, terminal, storage, observability, or security substrates inside GUI code.
- Adding a GUI framework as a drive-by dependency before ADR, dependency policy, and proof evidence.
- Letting UI own editor text, workspace state, proposal lifecycle state, storage, terminal sessions, provider calls, plugin hosts, collaboration sessions, or remote sessions.
- Claiming "fully functional IDE" from CLI proof, projection-only tests, or architecture documents alone.
- Autonomous AI mutation before proposal ledger, context manifest, approval, rollback, and audit surfaces are usable.

## Constraints
- Preserve the repository invariant from `AGENTS.md`: `legion-ui` emits `CommandDispatchIntent` and accepts snapshots; it must remain projection-only.
- Preserve proposal-mediated save flow: `AppComposition::save_active_buffer` -> `SaveWorkflowService` -> `WorkspaceActor::save_file_with_proposal`.
- Workspace saves must retain expected fingerprint, file content version, workspace generation, buffer version, snapshot id, non-zero correlation id, and causality id.
- Observability and storage default to metadata-only redaction and must reject invalid zero/nil event identifiers.
- Placeholder or gated surfaces must remain inert until ADR, dependency-policy entry, and contract tests exist.
- Renderer work must preserve large-file degraded mode and never require unbounded full-source GUI projection.
- Windows-first viability, IME, clipboard, focus, accessibility, platform menus, file dialogs, high-DPI behavior, and measurable input latency are renderer decision criteria.
- Phase gates remain: `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, and CI `cargo deny check`.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Design source | User selected the latest Legion exploration design after `$legion explore`. | `.planning/explorations/2026-05-26-gui-ide-roadmap-design.md` |
| Codebase map | User requested a full `legion:map`; source code exists and the generated dataset is fresh at initialization. | `.planning/CODEBASE.md` plus `.planning/codebase/` artifacts |
| Project track | The current source already has deep substrate but no real GUI renderer. | Roadmap focuses on GUI productization rather than rebuilding core substrate |
| First phase | Renderer dependency and planning truth must be reconciled before implementation. | Phase 1 is Baseline Reconciliation and Renderer Decision |
| UI boundary | Existing architecture and tests require UI to remain projection-only. | GUI adapter may render and collect input, but app/editor/workspace retain authority |
| Workflow mode | No custom workflow preference was provided during start. | Guided execution, Standard planning depth, Balanced cost profile |
| Plan counts | Phase plan counts are estimates. | Future `/legion:plan` may create as many tasks as needed for the phase |

## Architecture Influences
- Fresh map metadata: schema `2.0`, commit `b521ab5e64696b9017f02595183de9ed1614f8eb`, 141 mapped source/config/doc files, fingerprint `aa7e4fc1bdc9885f51b8fdf7ea44544239706d2dff6f02791f40c05d30e013d7`.
- `legion-app` is the composition root and current CLI shell proof. It should remain the authority layer for editor/workspace/proposal/security/storage/runtime behavior.
- `legion-ui` is a projection and intent crate, not a renderer or state owner.
- `legion-protocol` and `legion-app` are high fan-in monoliths; GUI work should avoid broad edits and prefer adapter seams.
- `xtask` and `plans/dependency-policy.md` are part of the architecture contract. GUI dependencies and crate edges need policy coverage before implementation.
- Renderer integration must reconcile `plans/adrs/ADR-0002-ui-editor-rendering.md`, `plans/spikes/SPIKE-001A-result.md`, and current source facts.

---
*Last updated: 2026-05-26 after initialization*
