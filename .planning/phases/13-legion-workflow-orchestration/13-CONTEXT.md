# Phase 13: Legion Workflow Orchestration -- Context

## Phase Goal

Coordinate a full Legion Workflow team across local or provider-backed models, with conflict/dependency tracking, verification, sign-off, and approval-gated merge.

## Requirements Covered

- **R-020 (Derived)**: Define Legion Workflow orchestration governance, activation policy, and accepted evidence paths before runtime behavior is extended.
- **R-021 (Derived)**: Add metadata-first protocol contracts for Legion workflow sessions, worker assignments, model backends, dependency edges, conflicts, verification gates, sign-off, and merge approvals.
- **R-022 (Derived)**: Extend `devil-agent` with a bounded Legion workflow coordinator that schedules local or provider-backed worker runs through existing proposal-only and provider-routing boundaries.
- **R-023 (Derived)**: Extend tracker and memory surfaces with metadata-only workflow run, dependency, conflict, verification, sign-off, and outcome records.
- **R-024 (Derived)**: Route Legion workflow sessions through `AppComposition` so all execution, verification, sign-off, and merge/apply paths remain app-owned, proposal-mediated, dirty-workspace-aware, and approval-gated.
- **R-025 (Derived)**: Expose Legion workflow command-center projections and desktop review actions without moving authority into `devil-ui` or `devil-desktop`.
- **R-026 (Derived)**: Archive deterministic Phase 13 evidence, final gate output, and user-facing limits without claiming unverified autonomous merge behavior.

## What Already Exists

- Phase 8 exposed a delegated task command center for plan-only review, blockers/refusals, proposal-preview links, and audit readiness while runtime activation stayed `NotEncoded`.
- Phase 9 added metadata-first command, artifact-ledger, verification-run, and system-graph projections for Delegates and Legion Workflows product modes.
- Phase 10 added directive, spec, task graph, execution session, evidence, and approval artifact DTOs with explicit raw-payload retention flags.
- Phase 11 added fail-closed delegated runtime activation states, security contracts, stop gates, and readiness evaluation.
- Phase 12 activated `devil-agent` as an isolated sandbox/proposal runtime and added `AppComposition::execute_delegated_task` for a single plan. That runtime must remain isolated and proposal-only; Phase 13 builds team orchestration on top of it, not around it.

## Hard Constraints

- `devil-ui` remains projection-only and may only carry snapshots plus command intents.
- `devil-desktop` may render and translate actions, but must not own workflow execution, worker sessions, provider calls, terminal execution, file writes, proposal lifecycle, tracker state, or memory retention.
- Generated or worker-authored edits must become proposals or proposal-preview metadata first. Main workspace mutation requires existing app/proposal authority and explicit approval.
- Local/provider model routing must preserve assisted-AI consent, redaction, provider health/cost labels, non-zero `CorrelationId`, non-nil `CausalityId`, and non-zero `EventSequence`.
- Conflict and merge handling must be metadata-first until an approval gate is satisfied. Dirty main workspace, stale proposal preconditions, missing verification evidence, or missing audit-before-success must block merge readiness.
- Raw prompts, raw source bodies, terminal output bodies, provider payloads, and raw worker logs are not persisted unless an explicit retention policy says so. Phase 13 evidence should use metadata and hashes.
- The current codebase map is orientation only; execution agents must read live source before edits because Phase 12 changes are present in the dirty worktree.

## Key Design Decisions

- **Architecture approach**: sequential, policy-first activation. Phase 13 has seven sequential waves because protocol, agent orchestration, tracking/memory, app routing, UI/desktop projection, and final evidence share high-risk authority boundaries.
- **Runtime shape**: introduce Legion workflow orchestration as a bounded coordinator over existing delegated-task primitives. Do not add direct main-workspace mutation, uncontrolled process execution, or provider invocation from UI/desktop.
- **Model backends**: model workers are represented as local or provider-backed metadata, with provider-backed work routed through existing assisted-AI/provider consent contracts rather than invoked directly by the coordinator.
- **Merge semantics**: merge readiness is a proposal/approval artifact, not an automatic git merge. Conflicts, stale preconditions, dirty workspace state, and missing evidence produce visible blockers.
- **Evidence**: final acceptance must include targeted contract tests, desktop workflow tests, `xtask` dependency/policy gate, and the standard repository gates before claiming Phase 13 complete.

## Plan Structure

- **Plan 13-01 (Wave 1)**: Governance And Activation Gate -- add ADR/policy/evidence scaffolding for Legion Workflow orchestration before code activation.
- **Plan 13-02 (Wave 2)**: Protocol Workflow Contracts -- add metadata-first DTOs, validators, projections, and contract tests.
- **Plan 13-03 (Wave 3)**: Agent Workflow Coordinator -- add bounded team scheduling, worker assignment, dependency/conflict metadata, and proposal-only worker outputs in `devil-agent`.
- **Plan 13-04 (Wave 4)**: Tracker Memory Evidence Ledger -- persist metadata-only workflow run, dependency, verification, sign-off, and outcome records in tracker/memory.
- **Plan 13-05 (Wave 5)**: App Workflow Composition -- route workflow sessions, verification gates, sign-off, and merge readiness through `AppComposition`.
- **Plan 13-06 (Wave 6)**: Desktop Workflow Command Center -- expose workflow projections and review actions through UI/desktop without authority creep.
- **Plan 13-07 (Wave 7)**: Evidence Final Gate And Readiness -- add final evidence, `xtask` checks, runbook updates, and acceptance proof.
