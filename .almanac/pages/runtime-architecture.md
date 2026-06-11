---
title: Runtime Architecture
summary: "Legion IDE is an app-owned Rust workspace where protocol DTOs and evidence gates keep UI, workspace mutation, AI, and advanced runtimes separated."
topics: [architecture, systems, stack]
sources:
  - id: workspace-manifest
    type: file
    path: Cargo.toml
    note: Shows the active workspace members and crate layout.
  - id: app-root
    type: file
    path: crates/legion-app/src/lib.rs
    note: Shows the composition root and the crate boundaries that `legion-app` wires together.
  - id: dependency-policy
    type: file
    path: plans/dependency-policy.md
    note: Defines allowed internal edges and phase-gated runtime activation.
status: active
verified: 2026-06-08
---
`[[crates/legion-app/src/lib.rs]]` is the authority layer for the product. It composes workspace, editor, protocol, security, storage, UI projection, indexing, AI, plugin, collaboration, remote, terminal, memory, tracker, and desktop-facing surfaces without pushing those authorities into the renderer layer [@app-root]. The workspace manifest shows this as a broad multi-crate Rust workspace rather than a monolith or a plugin host wrapped around one editor core [@workspace-manifest].

[[getting-started]] is the front door for this graph.

## What owns what

`legion-app` owns runtime composition and command routing. `legion-ui` exports typed projections and `CommandDispatchIntent` values only. `legion-project` owns the workspace actor, file tree, trust-aware path checks, watcher handling, and durable file mutation. `legion-editor` owns buffers, transactions, snapshots, undo/redo, save-request DTOs, and viewport-ready text state. `legion-protocol` carries the DTOs, identifiers, validation helpers, and shared product-mode/runtime-surface vocabulary that let these crates communicate without direct state sharing [@app-root].

This separation is enforced twice. The crate graph is constrained by `[[plans/dependency-policy.md]]`, and the code is tested through app-level workflows that route real operations across the boundaries instead of bypassing them [@dependency-policy].

## The recurring project rule

The repository keeps the same shape across unrelated features:

- UI and desktop render projections and emit intents.
- App composition interprets those intents and owns lifecycle state.
- Editor, workspace, security, storage, and advanced runtimes stay bounded behind protocol DTOs.
- Evidence and dependency policy decide which runtime slices are active, accepted, or still deferred [@dependency-policy].

That rule is why pages such as [[projection-only-ui-and-desktop]], [[workspace-save-workflow]], [[text-snapshots-and-degraded-mode]], [[assisted-ai-and-delegated-tasks]], and [[legion-workflow-orchestration]] all connect back to the same authority split.

## Active and gated surfaces

The current workspace contains real implementations for `legion-index`, `legion-agent`, `legion-tracker`, `legion-memory`, `legion-ai`, and `legion-ai-providers`, but each activation path is still constrained by product mode, dependency policy, protocol validation, and metadata-only audit defaults [@workspace-manifest] [@dependency-policy]. Future-facing or higher-risk surfaces such as plugin runtime authority, collaboration, remote transport hardening, retention, telemetry export, and terminal execution are present in code but remain governed by accepted evidence boundaries instead of becoming ambient authority.

Read [[phase-gates-and-evidence]] before widening a runtime surface. Read [[metadata-only-observability-and-storage]] before storing new data. Read [[bounded-runtime-surfaces]] before assuming a crate's presence means the corresponding feature is fully productized.
