# Desktop Adapter Boundary v0.1

## Scope

`devil-desktop` is the planned renderer-backed desktop adapter for Phase 2. It renders current app/UI projections in a native desktop window and translates native UI events into app-owned commands. It is not an editor engine, workspace actor, proposal service, storage repository, provider runtime, terminal runtime, plugin host, collaboration runtime, remote runtime, telemetry spool, or retention vault.

The adapter may depend on `devil-app`, `devil-ui`, `devil-protocol`, and policy-approved renderer crates. It must not make `devil-ui` depend on renderer crates or move app/editor/workspace authority into UI.

## Startup Flow

1. Process entry initializes the renderer and desktop window.
2. The adapter asks `AppComposition` to open a trusted workspace using the same trust and path authority used by the CLI proof.
3. The app produces the initial `ShellProjectionSnapshot`.
4. The adapter renders the snapshot through the renderer.
5. Native input, menus, file dialogs, close requests, and command palette actions are translated into `CommandDispatchIntent` or explicit app-level requests.
6. The app handles the request, mutates only through existing authorities, and produces a new projection.
7. The adapter replaces rendered view state from the new projection.
8. Frame timing instrumentation records input-to-paint and frame variance without persisting raw source or sensitive payloads.

## Projection In

The adapter consumes `ShellProjectionSnapshot` as display input. Projection fields include layout, explorer, active buffer viewport, status messages, proposal ledger, privacy inspector, context manifest, permission budget, approval checklist, checkpoint/rollback, assisted-AI, delegated-task, plugin, and collaboration projections.

The adapter may cache renderer layout resources, glyph data, scroll offsets, focus target ids, and accessibility node ids. These caches are renderer implementation details and must not become authoritative app state.

The active buffer projection is bounded display data. The adapter must not reconstruct full editor text from rendered rows, small previews, glyph caches, clipboard state, or accessibility labels.

## Intent Out

The primary outbound command type is `CommandDispatchIntent`.

Allowed outbound examples:

- `CommandDispatchIntent::OpenPath` for user-selected paths.
- `CommandDispatchIntent::Save` for save requests.
- `CommandDispatchIntent::Undo`, `Redo`, `Insert`, `Delete`, or `Replace` for editor input.
- Proposal preview, approval, rejection, apply, rollback, cancellation, and detail intents.
- Plugin, AI, collaboration, and delegated-task intents that are already represented by protocol projections.

The adapter must not apply these intents itself. It must route them to `AppComposition` or another explicit app-owned entry point.

## Allowed Side Effects

- Create and manage native windows.
- Allocate renderer resources.
- Capture keyboard, mouse, focus, IME, close, menu, and file-dialog events.
- Read and write OS clipboard data only as part of a user action routed through app-owned command handling.
- Publish an accessibility tree derived from projection metadata.
- Record renderer timing metrics and metadata-only diagnostics.

## Forbidden Side Effects

- Direct file saves, deletes, renames, or workspace mutations.
- Direct editor text ownership, full-text persistence, dirty-state ownership, undo/redo ownership, or snapshot identity authority.
- Direct proposal lifecycle mutation outside app/protocol authority.
- Direct provider calls or secret persistence.
- Direct telemetry spool mutation, retention vault mutation, remote workspace mutation, plugin host mutation, collaboration mutation, or terminal/process mutation.
- Bypassing proposal-mediated saves or conflict/stale/denial handling.

In short, `devil-desktop` is projection-only on input and intent-only on output for product state. It must not own editor state and must not own workspace state.

## Error And Recovery Semantics

- Renderer crash: terminate or restart the adapter without marking commands or saves successful.
- Lost focus: request a fresh projection and preserve app-owned selection/dirty state.
- Clipboard failure: surface an error and avoid emitting a mutation intent that depends on missing data.
- IME cancellation: discard incomplete composition unless the app receives a completed edit intent.
- File-dialog cancellation: emit no workspace mutation.
- Save rejection: display the app-returned conflict, stale, or denied outcome and preserve dirty text.
- Accessibility publication failure: record metadata-only diagnostics and continue visual rendering without claiming accessibility proof.

## Test Harness Requirements

Phase 2 must include:

- A startup smoke test for opening a trusted workspace.
- Projection rendering tests for layout, explorer, active buffer viewport, status, proposal, and trust surfaces.
- Intent routing tests proving user actions produce `CommandDispatchIntent` or app-owned requests.
- Save conflict tests proving stale/external overwrite outcomes remain rejected and dirty text is preserved.
- Renderer timing capture for p50/p95 input-to-paint and frame variance.
- Windows-focused smoke evidence for IME, clipboard, focus, high DPI, file dialogs, and accessibility.

## Phase 2 Entry Criteria

- ADR-0002 accepts the `eframe`/`egui` Phase 2 foundation path and fallback triggers.
- ADR-0030 accepts the `devil-desktop` ownership boundary.
- `plans/dependency-policy.md` allows renderer crates only in `devil-desktop`.
- `xtask` fails closed if renderer/windowing dependencies appear in `devil-ui`.
- `cargo run -p xtask -- check-deps` and targeted `xtask` renderer-gate tests pass.
