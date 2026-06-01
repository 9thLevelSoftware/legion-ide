# ADR-0019: Legion Workflow Orchestration

Status: Accepted for Phase 13 governance  
Date: 2026-06-01  
Scope: Phase 13 Legion Workflow orchestration activation boundary

## Context

Phase 13 introduces Legion Workflow orchestration: a full workflow team that can plan, delegate, verify, collect evidence, request sign-off, and prepare merge readiness. The existing product shell already exposes three user-facing modes: Manual, Delegates, and Legion Workflows. Earlier phases established projection-only UI, proposal-mediated mutation, assisted-AI provider routing, metadata-only evidence ledgers, and isolated delegated-task runtime primitives.

Phase 12 activated only an isolated delegated-task runtime for a single plan. That runtime is proposal-only: worker-authored edits become proposals or proposal-preview metadata before they can affect the main workspace. Phase 13 builds team coordination on top of those primitives. It does not authorize UI-owned workflow execution, desktop-owned provider calls, direct workspace mutation, uncontrolled terminal execution, or autonomous merge.

## Decision

Legion Workflow orchestration is approved only as a phase-gated, metadata-first coordinator until later evidence proves each boundary. The coordinator may describe workflow sessions, worker assignments, dependencies, conflicts, verification gates, sign-off records, and merge-readiness metadata. It may schedule work only through already approved delegated-task and assisted-AI routing boundaries.

The following constraints are accepted:

1. **Projection/request UI boundary**: `devil-ui` and `devil-desktop` may render workflow projections and emit app-owned intents. They must not own workflow sessions, worker state, provider calls, terminal processes, proposal lifecycle, tracker records, memory retention, or file mutation.
2. **Proposal-only worker outputs**: generated or worker-authored edits must become proposal metadata, proposal-preview metadata, or artifact-ledger evidence before any main-workspace mutation is possible.
3. **Approval-gated merge**: merge readiness is metadata and never an autonomous merge. Dirty workspace state, stale proposal preconditions, unresolved conflicts, missing verification, missing sign-off, missing rollback metadata, or missing audit-before-success evidence must block readiness.
4. **Provider routing through consent surfaces**: provider-backed workers must route through assisted-AI provider/consent metadata. The coordinator must not directly invoke providers or persist provider payloads.
5. **Metadata-only retention by default**: raw prompts, raw source bodies, raw diffs, terminal output bodies, provider request/response payloads, credentials, and raw worker logs are not persisted by Phase 13 workflow artifacts unless a future retention ADR explicitly authorizes it.
6. **Non-zero audit identity**: workflow session, worker, sign-off, provider-route, and evidence records must preserve non-zero `CorrelationId`, non-nil `CausalityId`, and non-zero event sequences wherever event ordering is represented.
7. **Fail-closed readiness**: workflow validators and readiness evaluators must reject or block incomplete metadata rather than infer success.

## Approved crate boundary

- `devil-protocol` may define metadata-first Legion Workflow DTOs, validators, projections, and readiness helpers.
- `devil-agent` may later coordinate workflow teams through isolated delegated-task primitives, but it must not acquire app, UI, editor, project, terminal, or desktop authority.
- `devil-tracker` and `devil-memory` may later store metadata-only workflow/evidence records under their existing storage boundaries and retention policies.
- `devil-app` may later own workflow execution state, verification/sign-off routing, dirty/stale/conflict blockers, and approval-gated merge readiness.
- `devil-ui` and `devil-desktop` may later expose command-center projections and app-request intents only.

## Forbidden behavior

The following remain explicitly unsupported in Phase 13:

- Autonomous merge or autonomous apply to the main workspace.
- UI or desktop mutation authority.
- Direct `devil-agent` mutation of editor buffers or workspace files.
- Provider calls initiated by UI/desktop or stored as raw payloads in protocol/tracker/memory.
- Terminal/process execution initiated outside terminal policy gates.
- Raw prompt/source/log retention by default.
- Dependency-policy edges from protocol to runtime crates or from UI to app/editor/project/storage.

## Merge readiness semantics

Approval-gated merge readiness is a conservative metadata evaluation. A session is ready only when all of the following are true:

- At least one worker assignment is valid and all blocking dependencies are satisfied.
- All conflicts are resolved or explicitly waived by metadata.
- Required verification gates have passed and link to evidence metadata.
- Required sign-offs are approved and link to approval metadata.
- Proposal identifiers exist for worker-authored changes.
- Main workspace is not dirty relative to workflow preconditions.
- Proposal preconditions are not stale.
- Audit-before-success metadata exists.
- Explicit merge approval and rollback/checkpoint metadata exist.

Any missing or invalid item blocks readiness. Readiness does not apply changes; it only indicates that app-owned proposal/approval authority may present the next user decision.

## Evidence required before Phase 13 completion

Phase 13 cannot be claimed complete until evidence includes:

- Protocol contract tests proving serializable metadata-only session, worker, dependency, conflict, verification, sign-off, projection, and readiness DTOs.
- Validator tests proving fail-closed behavior for invalid identifiers, zero schema versions, zero correlation, nil causality, unsafe redaction hints, provider-backed workers without route metadata, dirty workspace, stale proposals, unresolved conflicts, failed verification, missing sign-off, missing proposal ids, and missing audit-before-success evidence.
- Agent coordinator tests proving work is scheduled through delegated-task primitives and worker outputs are proposal-only.
- Tracker/memory tests proving metadata-only retention and consent-gated memory behavior.
- App workflow tests proving app-owned execution, verification, sign-off, dirty/stale/conflict blockers, and approval-gated merge readiness.
- UI/desktop tests proving projection-only command-center surfaces and app-request intents.
- Final `xtask` dependency-policy validation plus standard repository gates.

## Consequences

The project gains a clear activation path for Legion Workflows without weakening previously accepted safety boundaries. This ADR intentionally delays runtime authority until protocol, agent, tracker/memory, app, UI/desktop, and evidence waves land in order. It also gives reviewers concrete stop gates when a future patch attempts to bypass proposal-mediated mutation or approval-gated merge.
