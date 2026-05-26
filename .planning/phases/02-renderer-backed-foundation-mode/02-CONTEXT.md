# Phase 2 Context: Renderer-Backed Foundation Mode

## Workflow Inputs

- User command: `$legion plan 2 --auto-refine`.
- Project root: `C:\Users\dasbl\RustroverProjects\devil-ide`.
- Phase source: `.planning/ROADMAP.md`, phase 2, "Renderer-Backed Foundation Mode".
- Code map source: `.planning/CODEBASE.md` plus `.planning/codebase/index.jsonl` and `.planning/codebase/symbols.json`.
- Phase 1 readiness source: `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-REVIEW.md` and `plans/evidence/gui-productization/phase-1-renderer-readiness.md`.
- Structured Legion `AskUserQuestion` confirmation is unavailable in this Codex runtime. The direct user command and `--auto-refine` flag are treated as approval to generate and refine executable Phase 2 plans from current repository evidence.
- `.planning/REQUIREMENTS.md`, `.planning/memory/RETRO.md`, `.planning/memory/OUTCOMES.md`, and `.planning/settings.json` were absent at plan-generation time. Plan count follows roadmap estimate, and each plan stays at three concrete tasks or fewer.

## Current State Evidence

- Phase 1 review passed on 2026-05-26 after the renderer dependency gate was fixed to check every workspace package except `devil-desktop`.
- `plans/evidence/gui-productization/phase-1-renderer-readiness.md` accepts `eframe`/`egui` as the Phase 2 Windows-first foundation renderer proof path and records Slint/custom fallback criteria.
- `plans/adrs/ADR-0030-desktop-adapter-boundary.md` defines `devil-desktop` as an adapter that owns renderer resources, native input translation, metrics, and accessibility publication, but not editor/workspace/proposal authority.
- `plans/dependency-policy.md` authorizes renderer/windowing dependencies only in `devil-desktop`; `xtask/src/main.rs` enforces this through `validate_renderer_dependency_gate`.
- `devil-ui` remains projection-only. `crates/devil-ui/src/ui.rs` defines `ShellProjectionSnapshot`, `Shell`, and `CommandDispatchIntent`.
- `devil-app` owns app authority. `AppComposition::open_workspace`, `open_file`, `dispatch_ui_intent`, `save_active_buffer`, and `shell_projection_snapshot` are the approved runtime entry points for Phase 2.
- The current runnable product is still `crates/devil-app/src/main.rs`, a CLI shell proof that opens a trusted workspace and supports `:w`/`:q`; no `devil-desktop` crate exists before Phase 2.

## Phase Goal

Phase 2 must open a real desktop window, render current shell projections, and route all user actions through existing app authority.

The phase is successful only if:

- `devil-desktop` exists as the only renderer-backed crate or binary.
- The GUI consumes `ShellProjectionSnapshot` and renders layout, explorer, active buffer viewport, status, proposal summary, and trust summary.
- Keyboard, menu, path-dialog, save, edit, and quit actions become `CommandDispatchIntent` values or explicit app-owned requests.
- A user can open this repository, open a file, edit a small buffer, save, see save rejection/conflict state, and quit.
- `devil-ui` and core crates do not depend on renderer/windowing crates or `devil-desktop`.
- Renderer proof records input-to-paint, frame variance, focus, clipboard, IME, high-DPI, file-dialog, and accessibility smoke results.

## Waves

- Wave 1: Plan 02-01 scaffolds `devil-desktop`, workspace dependency wiring, and compile-safe module stubs.
- Wave 2: Plan 02-02 implements projection rendering; Plan 02-03 implements intent/app-request bridging. They run in parallel and write disjoint module/test files.
- Wave 3: Plan 02-04 connects the renderer shell to `AppComposition` for open/edit/save/rejection/quit workflows.
- Wave 4: Plan 02-05 adds renderer timing and platform smoke evidence capture.
- Wave 5: Plan 02-06 runs phase gates and writes the final Phase 2 evidence package.

## Non-Negotiable Constraints

- Preserve projection-only UI. `devil-ui` must not depend on `eframe`, `egui`, `devil-desktop`, `devil-app`, editor, project, storage, workspace, provider, terminal, telemetry, remote, plugin, collaboration, retention, or AI runtime crates.
- Preserve proposal-mediated saves. Save requests must route through `AppComposition::dispatch_ui_intent(CommandDispatchIntent::Save { .. })` or `AppComposition::save_active_buffer`; rejected saves must preserve dirty editor text.
- Preserve renderer dependency isolation. Only `devil-desktop` may declare renderer/windowing/accessibility crates authorized by ADR-0002 and dependency policy.
- Keep placeholder/gated crates inert. Do not activate AI/provider, plugin, collaboration, remote, terminal, telemetry, or retention behavior beyond projections already produced by `AppComposition`.
- Verify before reporting completion. Phase 2 cannot be accepted from screenshots or documentation alone if code gates, smoke tests, or renderer-boundary checks fail.

## Auto-Refine Summary

The `--auto-refine` pass found three high-impact planning risks and refined the plan set before finalization:

1. The initial decomposition could have left platform proof as manual-only. Plan 02-05 now requires an automated timed smoke mode and an evidence file with explicit p50/p95, frame variance, focus, clipboard, IME, high-DPI, file-dialog, and accessibility fields.
2. The initial decomposition mixed app authority routing with visual rendering. Plans 02-02 and 02-03 now split projection rendering from intent/app-request bridging so `devil-ui` cannot accidentally gain renderer or app ownership.
3. The initial scaffold did not make `xtask check-deps` a first-wave gate. Plan 02-01 now requires the workspace member, dependency policy compatibility, and renderer gate to pass before any rendering work proceeds.
