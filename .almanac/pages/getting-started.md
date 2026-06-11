---
title: Getting Started
summary: "Start here to navigate the wiki by authority boundary: app composition, projection-only UI, proposal-mediated saves, text snapshots, phase gates, and the bounded AI/runtime surfaces layered on top."
topics: [architecture, concepts]
sources:
  - id: workspace-manifest
    type: file
    path: Cargo.toml
    note: Confirms the workspace breadth behind the navigation guidance.
  - id: app-root
    type: file
    path: crates/legion-app/src/lib.rs
    note: Confirms that `legion-app` is the composition root that ties most pages together.
status: active
verified: 2026-06-08
---
This wiki is organized around the repository's control boundaries, not around the crate list. `legion-app` is the composition root for most runtime behavior, so most work starts from [[runtime-architecture]] and then branches into the specific authority boundary you are about to touch [@workspace-manifest] [@app-root].

## Read these first

- [[runtime-architecture]] for the overall crate split and the recurring "app owns authority, UI renders projections" rule.
- [[projection-only-ui-and-desktop]] if your change touches rendering, commands, panels, or desktop behavior.
- [[workspace-save-workflow]] if your change can mutate files, proposals, save behavior, or conflict handling.
- [[text-snapshots-and-degraded-mode]] if your change touches buffers, viewport rendering, large files, or snapshot consumers.
- [[phase-gates-and-evidence]] if you are widening a dependency edge, reviving a deferred runtime surface, or making product-readiness claims.

## Common work areas

For language features, read [[indexing-and-language-tooling]] after [[text-snapshots-and-degraded-mode]].

For AI, provider routing, or delegated execution, read [[assisted-ai-and-delegated-tasks]] and then [[metadata-only-observability-and-storage]].

For multi-worker execution and merge readiness, read [[legion-workflow-orchestration]] after [[assisted-ai-and-delegated-tasks]].

For plugin, collaboration, remote, terminal, telemetry, or retention work, read [[bounded-runtime-surfaces]] and then [[phase-gates-and-evidence]] before changing code.

## Dense cluster map

The densest cluster in this repo is the authority chain:

- projections and renderer adapters: [[projection-only-ui-and-desktop]]
- editor text and viewport contracts: [[text-snapshots-and-degraded-mode]]
- durable mutation and conflict handling: [[workspace-save-workflow]]
- audit/storage privacy defaults: [[metadata-only-observability-and-storage]]

The second cluster is the productized advanced runtime stack:

- semantic/index surface: [[indexing-and-language-tooling]]
- assisted AI and delegated tasks: [[assisted-ai-and-delegated-tasks]]
- Legion Workflow orchestration: [[legion-workflow-orchestration]]
- bounded plugin/remote/collaboration/terminal/retention/telemetry surfaces: [[bounded-runtime-surfaces]]

If search results feel noisy, return to these two clusters first. They explain most of the repo's non-obvious constraints.
