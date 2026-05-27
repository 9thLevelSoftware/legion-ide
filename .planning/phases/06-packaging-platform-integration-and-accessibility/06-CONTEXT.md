# Phase 6: Packaging, Platform Integration, and Accessibility -- Context

## Workflow Inputs

- Command: `$legion plan 6 --auto-refine`
- Roadmap phase: Phase 6, "Packaging, Platform Integration, and Accessibility"
- Requirements: R-012, R-013
- Requirements source: `.planning/ROADMAP.md` and `.planning/PROJECT.md`. `.planning/REQUIREMENTS.md` is absent.
- Planning mode: auto-refine enabled.
- Settings: no `settings.json` was found at the project root, so the workflow default of at most three implementation tasks per plan applies.
- Control modes: `.planning/config/control-modes.yaml` is absent, so workflow-common guarded defaults apply.
- GitHub issue creation: skipped because `git remote get-url origin` returned no origin remote.
- Agent directory: `C:/Users/dasbl/.legion/agents`; assigned agent ids were validated there.

## Codebase Map

The codebase map exists but is stale relative to live source:

- Map generated at `2026-05-27T12:57:55.9718684-04:00`
- Map commit: `beb896492685fadbb4d1669250f0a5f5a145f613`
- Current HEAD during planning: `f44932aeeeeaa6cc9c7521d0fed24227f10358a8`
- Worktree note: `.planning/CODEBASE.md`, `.planning/codebase/index.jsonl`, `.planning/codebase/search.md`, `.planning/codebase/symbols.json`, and `.planning/config/directory-mappings.yaml` were already modified before Phase 6 planning.

Use `.planning/CODEBASE.md` and `.planning/codebase/` for orientation only. Every build plan requires live source reads before edits.

## Phase Goal

Turn the renderer-backed GUI into an installable Windows desktop application with credible platform behavior.

Roadmap success criteria:

- Windows packaged executable or installer is produced.
- Native menus, file dialogs, clipboard, keyboard shortcuts, theme, focus traversal, IME, high-DPI behavior, and accessibility tree have smoke evidence.
- Crash-safe session restore and diagnostics export are available.
- Smoke-test scripts cover install, launch, open workspace, edit/save, terminal, LSP, proposal review, and quit.
- macOS/Linux parity plan and initial CI smoke coverage are documented.

## Current State Evidence

- `devil-desktop` already launches through `crates/devil-desktop/src/main.rs` and `crates/devil-desktop/src/workflow.rs`.
- `DesktopLaunchConfig` currently supports `--workspace`, `--file`, `--smoke`, `--duration-ms`, `--evidence`, and `--session-state`.
- `crates/devil-desktop/src/smoke.rs` records p50/p95 input-to-paint, frame variance, focus, clipboard, IME, high-DPI, file-dialog, accessibility, large-file degraded, bounded search, and full-text-avoidance fields. Some fields are still adapter-path or `not observed`, especially accessibility.
- `crates/devil-desktop/src/session.rs` persists metadata-only session JSON, but writes directly to the final path and does not yet provide explicit crash-safe temporary/write-then-rename semantics.
- `crates/devil-cli/src/main.rs` has diagnostics and evidence checks for older substrate phases, but it does not yet expose a GUI Phase 6 evidence check or a desktop diagnostics export command.
- `scripts/run-phase-gates.ps1` and `scripts/run-phase-gates.sh` run repository gates but do not exercise packaging or GUI smoke flows.
- `.github/workflows/ci.yml` already runs on Ubuntu, Windows, and macOS, but does not yet run GUI package dry-run or headless smoke validation.
- `xtask/src/main.rs` still treats Phase 6 as the legacy collaboration evidence gate through `plans/evidence/phase-6/collaboration-architecture-map.md`. The active GUI roadmap Phase 6 is different and must be given a distinct evidence path.

## Non-Negotiable Constraints

- `devil-ui` remains projection-only and must not gain renderer, app, editor, project, storage, terminal, provider, plugin, collaboration, or remote authority.
- `devil-desktop` may own packaging scripts, native window setup, renderer resources, adapter-local platform observations, diagnostics export formatting, and metadata-only session persistence. It must not own editor text, workspace mutation, proposal lifecycle authority, terminal runtime authority, provider routing, storage authority, or security policy.
- GUI packaging must not add a production dependency without a policy update and explicit justification. Prefer `cargo build --release -p devil-desktop` plus scripts before introducing installer tooling.
- Smoke evidence must distinguish OS-observed facts from adapter-path checks. Do not convert `not observed` into accepted proof without a deterministic check.
- Diagnostics export must remain metadata-only and reject raw source, prompt bodies, terminal payloads, provider payloads, and secret-like markers.
- Crash-safe session restore must preserve metadata-only behavior and must never persist dirty buffer text.
- Final acceptance cannot overwrite or delete the legacy accepted Phase 6 collaboration evidence under `plans/evidence/phase-6/`.

## Key Design Decisions

- Architecture proposals were skipped because the direct user command plus `--auto-refine` means generate executable plans from current source evidence.
- The roadmap estimate of five plans is treated as an estimate, not a cap. Seven waves are required because governance, packaging, platform smoke, session/diagnostics, script/CI coverage, evidence capture, and final acceptance each have distinct owners and verification.
- The first wave is governance because the current `xtask` Phase 6 gate points at accepted legacy collaboration evidence. GUI Phase 6 must use separate evidence under `plans/evidence/gui-productization/`.
- Packaging uses a Windows release package script and manifest first, not a new installer dependency. A future installer can be added only after the package path is verified and policy-approved.
- Platform smoke is split from final evidence. Build agents must improve deterministic observation where feasible, while preserving honest `not observed` reporting when OS-level evidence is unavailable.
- Crash-safe session persistence and diagnostics export are grouped because both are metadata-only reliability surfaces and must reject raw-source markers.
- Final acceptance is its own wave and may update roadmap/state only after full gates and GUI Phase 6 evidence pass.

## Plan Structure

- **Plan 06-01 (Wave 1)**: GUI Phase 6 Governance And Evidence Gate -- add a distinct GUI Phase 6 packaging/platform/accessibility acceptance path without changing legacy collaboration Phase 6 evidence.
- **Plan 06-02 (Wave 2)**: Windows Package And Launch Contract -- add a Windows package manifest builder, dry-run capable packaging script, and package runbook.
- **Plan 06-03 (Wave 3)**: Native Platform Integration Smoke Model -- add deterministic platform integration snapshotting for menus, file dialogs, clipboard, shortcuts, theme, focus, IME, high-DPI, and accessibility tree smoke rows.
- **Plan 06-04 (Wave 4)**: Crash-Safe Session And Diagnostics Export -- make session writes crash-safe and add metadata-only diagnostics export with raw-payload rejection.
- **Plan 06-05 (Wave 5)**: GUI Smoke Scripts And CI Coverage -- add install/launch/workflow smoke scripts, GUI Phase 6 CLI evidence check, and initial CI dry-run smoke coverage.
- **Plan 06-06 (Wave 6)**: Phase 6 Evidence Capture And Parity Plan -- archive package, platform, accessibility, session, diagnostics, smoke, performance, and cross-platform parity evidence.
- **Plan 06-07 (Wave 7)**: Phase 6 Acceptance Gate -- run final gates, mark GUI Phase 6 accepted only with complete evidence, and update planning state.

## Auto-Refine Summary

The `--auto-refine` pass identified and addressed these planning risks before finalization:

1. Existing Phase 6 evidence is accepted collaboration-substrate evidence, not GUI packaging evidence. Plan 06-01 creates a separate GUI Phase 6 gate and forbids deleting legacy evidence.
2. Packaging could drift into unapproved installer dependencies. Plan 06-02 starts with a release executable package and dry-run script, and blocks new dependencies unless policy is updated.
3. Current smoke output includes platform fields but accessibility remains `not observed`. Plan 06-03 requires deterministic accessibility-tree smoke data and honest OS-observed versus adapter-path labels.
4. Session restore is metadata-only but not crash-safe. Plan 06-04 requires temporary-file write, validation, and atomic rename where the platform supports it.
5. Existing scripts run cargo gates only. Plan 06-05 adds GUI workflow smoke scripts and CI dry-run coverage for package/smoke paths.
6. Final acceptance needs evidence across packaging, platform behavior, accessibility, diagnostics, session restore, smoke workflows, and full gates. Plans 06-06 and 06-07 separate evidence capture from acceptance.
