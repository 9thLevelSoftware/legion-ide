# Dependency Policy for Devil IDE v0.1

## Scope

This document defines the required crate dependency direction used by `cargo run -p xtask -- check-deps` during milestone-gate validation.

## Rules

### 1. Directional Intent

- `devil-ai` may depend on:
  - `devil-protocol`
  - `devil-security`
  - `serde`, `serde_json`, `thiserror`
- `devil-ai` MUST NOT depend on `devil-ai-providers`.

- `devil-ai-providers` may depend on:
  - `devil-ai`
  - `devil-protocol`
  - `devil-security`

- `devil-editor` may depend on:
  - `devil-text`
  - `devil-protocol`

- `devil-editor` MUST NOT depend on `devil-project`.

- `devil-ui` may depend on:
  - `devil-editor`
  - `devil-protocol`

- `devil-platform` may depend on:
  - `devil-protocol`
  - `thiserror`

### 2. Shared Contracts Boundary

- Cross-domain project/editor/indexer/tracker interactions should flow through `devil-protocol` types and traits.
- The following boundary API symbols are authoritative in `devil-protocol`:
  - `ProjectId`, `FileId`, `BufferId`, `SnapshotId`
  - `ProjectInfoQuery`, `ProjectInfo`, `EditorTransactionEvent`, `BufferOpened`
  - `ProjectInfoPort`

### 3. Forbidden/Deferred Edges (Milestone 0)

- Do not add hard edges from:
  - `devil-editor` -> `devil-project`
  - `devil-ui` -> feature crates beyond declared contracts
  - `devil-tracker` -> feature crates that are not storage-protocol mediated
  - `devil-memory` -> non-storage non-protocol feature domains without explicit planning

### 4. Enforcement

`xtask check-deps` reads this policy and fails when forbidden edges are detected.
