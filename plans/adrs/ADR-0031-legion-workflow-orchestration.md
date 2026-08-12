# ADR 0031: Legion Workflow Orchestration

## Status
Accepted

## Context
Phase 13 introduces Legion Workflow orchestration. We need to coordinate a team of workers (local or provider-backed), model dependency execution order, track conflict metadata, require verification and sign-off, and ultimately achieve a merge-ready state. This must be done without granting UI/desktop authority, allowing direct main-workspace mutation, or permitting autonomous merge before approval.

## Decision
We will model Legion Workflow orchestration using metadata-first protocol DTOs.
- `legion-agent` will serve as the bounded coordinator for local/provider-backed workers.
- `AppComposition` will own workflow session lifecycle, verification routing, and approval-gated merge readiness.
- UI and desktop interfaces remain projection-only.

## Architecture
- `legion-protocol`: Defines metadata-only DTOs for `LegionWorkflowSession`, workers, dependencies, conflicts, verification, sign-off, and merge readiness.
- `legion-agent`: Implements `LegionWorkflowCoordinator` utilizing isolated `DelegatedTaskSandboxOrchestrator` for local workers and route-requests for provider-backed workers.
- `legion-tracker` & `legion-memory`: Store outcome candidates and records using metadata boundaries, restricted by consent constraints.
- `legion-app`: Governs execution state, verifies conditions (stale, dirty, missing sign-off), and acts as the authority for workflow progress.
- `legion-ui` & `legion-desktop`: Present summary projections and relay app request intents.

## Model Backend Policy
Legion workflow workers may be represented as local or provider-backed. Provider-backed execution is routed through assisted-AI/provider consent metadata and cannot be invoked from UI/desktop.

## Conflict And Dependency Tracking
Dependencies are tracked logically via metadata (worker order). Conflicts are detected via `files_modified` overlaps. Unresolved conflicts block merge readiness without mutating main workspace or removing output until resolved by the app composition.

## Verification And Sign-Off
Verification (e.g. test gates) and manual/peer sign-off are required metadata fields. Completed sessions without verification and sign-off evidence are blocked.

## Approval-Gated Merge
Approval-gated merge is defined as a proposal/approval readiness state. Main-workspace mutation is forbidden until proposal approval, clean/stale checks, evidence checks, and audit-before-success checks all pass.

## Security/Privacy
- Autonomous merge is not supported.
- Raw prompt, source, logs, and provider payloads are not retained by default.
- UI/desktop do not own runtime authority.
- Direct workspace mutation is forbidden.

## Alternatives Considered
- *Full autonomous multi-agent execution*: Rejected due to safety and control loss.
- *App-level worker scheduling*: Rejected as it mixes protocol-agnostic execution with application state. Placing coordination in `legion-agent` while keeping app composition strictly authoritative over transitions isolates side effects.

## Consequences
- Workflow orchestration introduces new metadata types but zero new runtime mutators.
- Adding provider-backed capabilities requires proper metadata routing and assisted-AI constraints.
- UI implementations remain purely projection-based.

## Acceptance Evidence
- Acceptance is determined by final phase gates enforcing `Phase 13` dependency boundaries and passing `cargo check`/`cargo test` suites against `legion-protocol`, `legion-agent`, `legion-tracker`, `legion-memory`, `legion-app`, and `legion-desktop`.
