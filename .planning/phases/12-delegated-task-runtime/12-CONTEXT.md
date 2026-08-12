# Phase 12 Context: Delegated Task Runtime

## Goal
Activate `devil-agent` as an isolated orchestrator that emits proposals and evidence, never direct main-workspace mutation.

## Phase Requirements
- **R-016 (Derived)**: Isolated Sandbox Orchestrator inside `devil-agent` using git worktrees or isolated subdirectories under `target/`.
- **R-017 (Derived)**: Mutation-safe `AssistedAiEditProposalOutput` proposal generation.
- **R-018 (Derived)**: App-gated composition enforcing `DelegatedTaskRuntimeActivationState` transitions.
- **R-019 (Derived)**: End-to-end integration and desktop projection mapping for reviews and approvals.

## Existing Assets
- `crates/devil-protocol/src/lib.rs`: Full set of public metadata DTOs, activation state enums, audit records, and validations.
- `crates/devil-agent/src/lib.rs`: Agent run state machine and replay capability foundation.
- `crates/devil-app/src/lib.rs`: Active document state, proposal-mediated save workflows, and seed projection contracts.
- `crates/devil-desktop/src/`: bridge and workflow presentation layers with inspection routing.

## Key Design Decisions
- **Architecture Approach: Pragmatic / Clean Hybrid**
  - Run all task executions inside temporary git worktrees (`target/delegated-tasks/task-{id}`) to leverage git's native diffing and isolation capabilities.
  - Fail-closed security gates: Any attempt to read or modify files outside the declared boundaries must immediately halt execution with a `BLOCKED` status.
  - UI/Desktop remains projection-only; approvals must route through `AppComposition` to ensure authorization checks are fully enforced.

## Plan Structure
- **Wave 1**:
  - **Plan 12-01**: Isolated Sandbox and Proposal Generator in `devil-agent`
- **Wave 2**:
  - **Plan 12-02**: Gated Runtime Composition and Policy Gating in `devil-app`
- **Wave 3**:
  - **Plan 12-03**: End-to-End Desktop Integration and Verification Harness
