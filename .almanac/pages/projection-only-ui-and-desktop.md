---
title: Projection-Only UI And Desktop
summary: "`legion-ui` and `legion-desktop` are intentionally non-authoritative layers that render snapshots and relay typed intents back into `legion-app`."
topics: [ui-shell, architecture, decisions, productization]
sources:
  - id: ui-lib
    type: file
    path: crates/legion-ui/src/lib.rs
    note: Shows the public projection and intent surface exported by the UI crate.
  - id: desktop-lib
    type: file
    path: crates/legion-desktop/src/lib.rs
    note: States the desktop adapter boundary and native-resource ownership.
  - id: desktop-bridge
    type: file
    path: crates/legion-desktop/src/bridge.rs
    note: Shows adapter-local actions before they are routed into app-owned commands.
  - id: adr-0002
    type: file
    path: plans/adrs/ADR-0002-ui-editor-rendering.md
    note: Records the accepted renderer direction and the non-authoritative UI rule.
status: active
verified: 2026-06-08
---
`[[crates/legion-ui/src/lib.rs]]` re-exports projection structs, layout/view state, and `CommandDispatchIntent`. It does not expose editor engines, workspace actors, save authorities, or renderer dependencies [@ui-lib]. `[[crates/legion-desktop/src/lib.rs]]` explicitly says native windowing and renderer resources live in the desktop adapter while product state stays owned by `legion-app`, `legion-ui`, and `legion-protocol` [@desktop-lib].

## Why the rule exists

`[[plans/adrs/ADR-0002-ui-editor-rendering.md]]` keeps the repository away from VS Code-style state splits. The accepted direction is a Rust-native renderer path, but the renderer is still an adapter. The ADR names the invariant directly: `legion-ui` consumes protocol projections and emits intents; it must not own editor text, workspace state, save decisions, provider state, telemetry storage, or persistence policy [@adr-0002].

## What the desktop adapter is allowed to do

`[[crates/legion-desktop/src/bridge.rs]]` defines `DesktopAction` values such as save, tab switching, explorer selection, proposal lifecycle requests, AI requests, terminal actions, delegated-task actions, workflow actions, and search prompts. These are adapter-local actions before app routing, not direct mutations [@desktop-bridge]. The desktop layer can own:

- native windows and renderer resources
- adapter-local prompts and expansion state
- file-dialog results and other OS bridge outputs
- translation from UI events into typed intents or app requests

It cannot make dirty-state, proposal-state, or workspace-state decisions on its own [@desktop-lib] [@desktop-bridge].

## What this protects

This boundary preserves the contracts in [[workspace-save-workflow]] and [[text-snapshots-and-degraded-mode]]. A renderer crash or UI bug must not imply a successful save, accepted proposal, AI dispatch, or workspace mutation. That is why save routing, proposal application, AI product modes, and workflow approval all terminate in app-owned code rather than in view code [@adr-0002].

The desktop crate is therefore a renderer-backed adapter, not a second application core.
