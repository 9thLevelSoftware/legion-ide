---
title: Text Snapshots And Degraded Mode
summary: "The text/editor stack is rope-backed and snapshot-based, with a 5 MiB full-cache budget and viewport-only degraded behavior for large files."
topics: [editor, stack, decisions, flows]
sources:
  - id: text-lib
    type: file
    path: crates/legion-text/src/lib.rs
    note: Defines the rope-backed text model, snapshots, and the full-cache budget.
  - id: editor-lib
    type: file
    path: crates/legion-editor/src/lib.rs
    note: Defines save DTOs, transaction records, snapshot leases, and degraded-save behavior.
  - id: ui-shell
    type: file
    path: crates/legion-ui/src/ui.rs
    note: Exposes `small_buffer_text` and large-file degraded projection messaging.
  - id: viewport-adr
    type: file
    path: plans/adrs/ADR-0015-streaming-text-viewport.md
    note: Defines the accepted viewport-only guardrails and save compatibility limits.
  - id: workspace-vfs-test
    type: file
    path: crates/legion-app/tests/workspace_vfs_integration.rs
    note: Verifies small-buffer and large-file projection behavior.
status: active
verified: 2026-06-08
---
`[[crates/legion-text/src/lib.rs]]` keeps text rope-backed and immutable by snapshot. The load-bearing threshold is `DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES = 5 * 1024 * 1024`, which controls whether a snapshot retains a compatibility full-text cache or stays chunked and sliceable only [@text-lib].

## Operating modes

The accepted Phase 1 rule in `[[plans/adrs/ADR-0015-streaming-text-viewport.md]]` is that UI gets full source only in explicitly bounded small-buffer mode. Large-file degraded mode must use viewport slices only, surface visible limitations, and avoid unbounded whole-file behavior [@viewport-adr].

The current tests match that rule:

- small files can project `small_buffer_text()` and render in normal viewport mode [@workspace-vfs-test]
- large files do not expose `small_buffer_text()` and force degraded behavior [@workspace-vfs-test]
- `legion-ui` carries explicit large-file messaging such as `Large file degraded mode: viewport payloads are chunked` [@ui-shell]

## Why the editor owns snapshots

`[[crates/legion-editor/src/lib.rs]]` keeps transaction records, snapshot descriptors, snapshot leases, undo/redo retention, and save-request DTOs in the editor layer rather than in UI or app view code [@editor-lib]. This lets later consumers such as indexing, LSP-style language tooling, AI retrieval, and observability reference snapshot metadata without regaining ownership of live text state.

## Save compatibility limit

The repository has not replaced saves with a streaming write path. `EditorError::DegradedSaveUnavailable` exists because current saves still need the editor to assemble a full payload inside the app/workspace save workflow boundary when that is allowed [@editor-lib]. The ADR treats that as an accepted limitation: large degraded files may fail closed rather than widening authority or leaking full text to UI [@viewport-adr].

This page is the text-side counterpart to [[workspace-save-workflow]].
