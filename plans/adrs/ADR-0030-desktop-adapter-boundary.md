# ADR-0030: Desktop Adapter Boundary

## Status

Accepted.

## Context

Phase 1 established that the existing product substrate is accepted through Phase 8 and that GUI productization is a new renderer-backed track. `legion-app` remains the app authority and current CLI proof. `legion-ui` remains projection-only: it accepts snapshots and emits `CommandDispatchIntent` values without mutating editor or workspace state.

ADR-0002 now selects `eframe`/`egui` for the Phase 2 Windows-first desktop foundation proof. The renderer decision does not authorize renderer dependencies in `legion-ui` or core crates.

## Decision

Introduce a planned desktop adapter crate named `legion-desktop` in Phase 2.

`legion-desktop` is an adapter, not a new authority layer. It may own the process entry point, native window, renderer resources, frame timing instrumentation, native input translation, clipboard bridge, file-dialog request forwarding, focus/IME integration, and accessibility tree publication. It must consume app/UI projections and route user actions back to app-owned command handling.

The adapter must not own editor text, workspace state, save decisions, proposal lifecycle state, provider credentials, telemetry storage, retention policy, terminal sessions, collaboration sessions, remote workspace authority, plugin host authority, or durable application persistence.

## Allowed Dependencies

`legion-desktop` may depend on:

- `legion-app`
- `legion-ui`
- `legion-protocol`
- Renderer crates authorized by ADR-0002 and `plans/dependency-policy.md`, initially `eframe` and `egui` for the Phase 2 foundation proof.

Core crates must not depend on `legion-desktop`. `legion-ui` must not depend on `legion-desktop`, `legion-app`, renderer/windowing crates, editor, project, storage, provider, telemetry, terminal, remote, retention, plugin, collaboration, or AI runtime crates.

## Forbidden Ownership

- `legion-desktop` must not own editor buffers, editor sessions, dirty state, undo/redo history, save proposals, file fingerprints, workspace generation, or path authority.
- `legion-desktop` must not write files directly. File dialogs may collect user intent, but the resulting open/save operations must route through app/workspace authority.
- `legion-desktop` must not apply proposals, code actions, AI edits, plugin outputs, terminal outputs, collaboration changes, or remote changes without app-owned approval and proposal workflows.
- `legion-desktop` must not persist raw source, provider payloads, terminal output, secrets, or telemetry records outside existing metadata-only storage and observability contracts.

## Security And Persistence Constraints

The adapter is a trust-boundary surface because it bridges native OS input, clipboard, IME, file dialogs, accessibility publication, and renderer callbacks into app-owned commands. Every action that can mutate files, buffers, terminal state, provider state, plugin state, collaboration state, or remote state must cross an app/protocol boundary with non-zero correlation and causality metadata where applicable.

Renderer crashes, lost focus events, failed clipboard reads, IME cancellation, or accessibility publication failures must not imply command success, file save success, proposal approval, or telemetry persistence. The adapter may surface errors, request retry, or close safely; it may not invent success-shaped state.

## Consequences

- Phase 2 can scaffold a native desktop shell without weakening the projection-only UI invariant.
- Renderer dependencies have one planned home: `legion-desktop`.
- `xtask` and dependency policy can fail closed if `legion-ui` or core crates gain renderer dependencies.
- Later editor-grade rendering can replace or deepen the adapter implementation without changing app/workspace/editor ownership.

## Verification

- `plans/desktop-adapter-boundary-v0.1.md` specifies startup, projection, intent, side-effect, and failure semantics.
- `plans/dependency-policy.md` documents the `legion-desktop` boundary and renderer dependency gate.
- `xtask/src/main.rs` includes `renderer_dependency_gate_preserves_projection_boundary`.
- Phase 2 must prove p50/p95 input-to-paint, IME, clipboard, focus, high-DPI, and accessibility behavior before claiming renderer-backed GUI acceptance.
