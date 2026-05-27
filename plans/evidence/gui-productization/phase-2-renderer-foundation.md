# Phase 2 Renderer Foundation

## Phase 2 renderer foundation: Accepted

Decision date: 2026-05-26

Phase 2 is accepted for renderer-backed foundation mode. The accepted scope is a real desktop adapter that launches a native eframe window, renders current `devil-ui` shell projections, translates renderer actions into app-owned commands or app requests, and records bounded renderer smoke evidence. This is not acceptance of the daily editing MVP, full native platform integration, or accessibility proof; those remain Phase 3 and Phase 6 obligations.

## Artifact Inventory

| Artifact | Status | Evidence |
| --- | --- | --- |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-05-RESULT.md` | Complete | Phase 1 readiness accepted for Phase 2; dependency, format, workspace check, UI/app check, and renderer-gate evidence recorded. |
| `.planning/phases/02-renderer-backed-foundation-mode/02-01-RESULT.md` | Complete | `devil-desktop` workspace crate added with renderer dependencies limited to the desktop adapter and verified by `xtask check-deps`. |
| `.planning/phases/02-renderer-backed-foundation-mode/02-02-RESULT.md` | Complete | Projection renderer panels implemented and covered by projection rendering tests. |
| `.planning/phases/02-renderer-backed-foundation-mode/02-03-RESULT.md` | Complete | Desktop action bridge implemented with no app/editor/workspace internals in the bridge. |
| `.planning/phases/02-renderer-backed-foundation-mode/02-04-RESULT.md` | Complete | `DesktopRuntime` routes open/edit/save/rejection/quit through `AppComposition` and app-owned command dispatch. |
| `.planning/phases/02-renderer-backed-foundation-mode/02-05-RESULT.md` | Complete | Timed renderer smoke harness and platform smoke evidence recorded. |
| `plans/evidence/gui-productization/phase-2-renderer-smoke.md` | Present | Smoke status passed with timing and platform fields. |

## Boundary Evidence

- `crates/devil-desktop/Cargo.toml` is the only crate manifest in the accepted Phase 2 surface that declares `eframe` and `egui`.
- `plans/dependency-policy.md` explicitly authorizes renderer/windowing dependencies only for `devil-desktop` and forbids renderer/windowing crates in `devil-ui` and core crates.
- `xtask/src/main.rs` enforces the renderer dependency gate; the live `cargo run -p xtask -- check-deps` gate passed.
- `crates/devil-desktop/src/view.rs` derives `DesktopProjectionViewModel` only from `ShellProjectionSnapshot` and renders explorer, active buffer, status, proposal, trust, assistant, plugin, delegated-task, and collaboration rows without taking product-state ownership.
- `crates/devil-desktop/src/bridge.rs` maps `DesktopAction` values into `CommandDispatchIntent`, `DesktopAppRequest`, `Noop`, or typed `DesktopBridgeError`; inverted source checks from Plan 02-03 found no `AppComposition`, `WorkspaceActor`, or `EditorEngine` dependency in the bridge.
- `crates/devil-desktop/src/workflow.rs` owns the adapter runtime and routes workspace open, file open, edits, saves, rejections, refresh, and quit through `AppComposition`/`dispatch_ui_intent` or adapter-local shutdown.
- `deny.toml` now documents the reviewed Phase 2 renderer transitive license set. `cargo deny check` exits 0 after that policy remediation, with duplicate-crate warnings still reported at warning level.

## Workflow Evidence

- Native launch path: `cargo run -p devil-desktop -- . Cargo.toml`.
- Smoke launch path: `cargo run -p devil-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 1500 --evidence plans/evidence/gui-productization/phase-2-renderer-smoke.md`.
- `DesktopLaunchConfig` opens a workspace with the desktop principal and can open an initial file through `AppComposition`.
- `DesktopRuntime::handle_action` translates UI actions through `DesktopCommandBridge`, dispatches app-owned intents through `AppComposition::dispatch_ui_intent`, and refreshes projection snapshots after every handled action.
- The regression `desktop_workflow_external_overwrite_save_rejects_and_preserves_dirty_projection` proves external overwrite between open and save yields `SaveRejected`, keeps disk content from being clobbered, and preserves dirty projected editor text.

## Renderer Smoke Evidence

Source: `plans/evidence/gui-productization/phase-2-renderer-smoke.md`.

| Field | Value |
| --- | --- |
| status | passed |
| workspace | `.` |
| file | `Cargo.toml` |
| duration_ms | 1500 |
| sample_count | 1 |
| p50_input_to_paint_ms | 3.120 |
| p95_input_to_paint_ms | 3.120 |
| frame_count | 127 |
| average_frame_ms | 11.884 |
| frame_variance_ms2 | 1027.753 |
| focus_smoke | os-observed focused |
| clipboard_smoke | adapter-path passed |
| ime_smoke | adapter-path passed |
| high_dpi_smoke | os-observed scale 1.500 |
| file_dialog_smoke | adapter-path passed |
| accessibility_smoke | not observed |

## Gate Results

Command summary:

- cargo run -p xtask -- check-deps: passed
- cargo fmt --all --check: passed
- cargo check --workspace --all-targets: passed
- cargo test -p devil-desktop --all-targets: passed
- cargo test --workspace --all-targets: passed
- cargo clippy --workspace --all-targets -- -D warnings: passed
- cargo deny check: passed with warning-level duplicate-crate findings
- Plan 02-05 timed smoke command: passed

| Command | Result | Notes |
| --- | --- | --- |
| `cargo run -p xtask -- check-deps` | passed | Dependency policy checks passed. |
| `cargo fmt --all --check` | passed | No formatting diff after the Phase 2 formatting remediation. |
| `cargo check --workspace --all-targets` | passed | Workspace all-target check completed. |
| `cargo test -p devil-desktop --all-targets` | passed | Desktop workflow, bridge, platform smoke, and projection rendering tests passed. |
| `cargo test --workspace --all-targets` | passed | Workspace tests passed; three performance-suite workloads remain ignored by design. |
| `cargo clippy --workspace --all-targets -- -D warnings` | passed | Warning-clean under clippy. |
| `cargo deny check` | passed with warnings | Advisories, bans, licenses, and sources ok; duplicate-crate warnings remain warning-level for the renderer/winit graph. |
| `cargo run -p devil-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 1500 --evidence plans/evidence/gui-productization/phase-2-renderer-smoke.md` | passed | Recorded in Plan 02-05 smoke evidence; not rerun in 02-06 because the committed smoke evidence was present and complete. |

## Success Criteria Decision

| Phase 2 criterion | Decision | Evidence |
| --- | --- | --- |
| Renderer-backed crate or binary launches a native window. | met | `devil-desktop` native launch path exists and timed eframe smoke command passed. |
| GUI consumes `ShellProjectionSnapshot` and renders layout, explorer, active buffer viewport, status, proposal summary, and trust summary. | met | `view.rs` builds `DesktopProjectionViewModel` from `ShellProjectionSnapshot`; `projection_rendering` tests passed. |
| Input/key/menu/file-dialog actions become `CommandDispatchIntent` or explicit app requests. | met | `bridge.rs` maps desktop actions into `CommandDispatchIntent` or `DesktopAppRequest`; `intent_bridge` tests passed. |
| User can open this repository, open a file, edit a small buffer, save, see conflict/rejection state, and quit. | met | `desktop_workflow` tests passed, including open/edit/save rejection preservation and quit. |
| UI code does not depend on editor/project/storage internals beyond approved projection/protocol contracts. | met | Renderer dependencies are isolated to `devil-desktop`; bridge inverted source check found no direct app/editor/workspace internals; `xtask check-deps` passed. |
| Renderer proof records input-to-paint, frame variance, focus, clipboard, IME, and accessibility smoke results. | met with explicit limitation | Smoke evidence records required timing and platform fields. Accessibility is recorded as `not observed`, not claimed as proven. |

## Residual Risks

- Foundation mode is a launchable proof shell, not the daily editing MVP. Multi-tab editing, repeated-session behavior, search, close-dirty prompts, and mature external-overwrite UX are Phase 3 work.
- Clipboard, IME, and file-dialog checks are adapter-path smoke in Phase 2. They prove routing paths, not full OS-level interaction fidelity.
- Accessibility smoke is explicitly `not observed`. Phase 6 must provide accessibility tree evidence before any platform-integration claim.
- `cargo deny check` exits 0 but still reports duplicate-crate warnings in the renderer/windowing/transitive graph. This is policy-permitted today and should be monitored as renderer dependencies evolve.
- The smoke run records `frame_variance_ms2: 1027.753`; Phase 2 treats this as launch evidence, not a user-facing performance budget.

## Phase 3 Entry Criteria

- Preserve the projection-only UI and proposal-mediated save boundaries while adding daily editing controls.
- Add multi-tab editor behavior, close/reopen behavior, save-all, and close-dirty prompts.
- Add repeated-session restore for workspace, tabs, focus, layout, and explorer state.
- Add search-in-file and workspace search through approved projections/services.
- Improve external overwrite/conflict presentation without marking rejected dirty text clean.
- Preserve large-file degraded behavior; GUI code must not require unbounded full-source projection.
