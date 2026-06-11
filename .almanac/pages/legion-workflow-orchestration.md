---
title: Legion Workflow Orchestration
summary: "Legion Workflow coordinates local or provider-backed workers through metadata-first app authority, with verification, sign-off, and merge readiness blocking autonomous mutation."
topics: [workflow-orchestration, ai-runtime, decisions, flows]
sources:
  - id: workflow-adr
    type: file
    path: plans/adrs/ADR-0031-legion-workflow-orchestration.md
    note: Records the accepted architecture and approval-gated merge rules.
  - id: agent-lib
    type: file
    path: crates/legion-agent/src/lib.rs
    note: Defines the bounded coordinator and worker/runtime state machine.
  - id: app-lib
    type: file
    path: crates/legion-app/src/lib.rs
    note: Composes workflow coordinator, tracker, memory, and UI projections.
  - id: workflow-tests
    type: file
    path: crates/legion-app/tests/legion_workflow_integration.rs
    note: Verifies local/provider-backed workflow sessions, MCP tool routing, and merge-readiness behavior.
status: active
verified: 2026-06-08
---
Legion Workflow is the repository's multi-worker orchestration layer, but it is deliberately metadata-first. `[[plans/adrs/ADR-0031-legion-workflow-orchestration.md]]` assigns worker coordination to `legion-agent`, session lifecycle and approval-gated transitions to `legion-app`, and keeps UI/desktop projection-only [@workflow-adr].

## Core rule

Workers may be local or provider-backed, but they do not get direct main-workspace mutation authority. Verification gates, sign-off records, conflict metadata, and merge approval all have to line up before a session becomes merge-ready, and autonomous merge is explicitly unsupported [@workflow-adr].

## Code shape

`[[crates/legion-agent/src/lib.rs]]` defines the workflow coordinator and worker-side contracts. `legion-app` imports `LegionWorkflowCoordinator`, delegated-task proposal generation, sandbox orchestration, memory service, and tracker ledgers, then projects the results through protocol DTOs [@agent-lib] [@app-lib]. This keeps workflow execution, evidence, and review state visible without pushing runtime authority into the UI layer.

## Proven behavior

`[[crates/legion-app/tests/legion_workflow_integration.rs]]` exercises:

- local and provider-backed worker assignments [@workflow-tests]
- verification and sign-off state in merge readiness [@workflow-tests]
- MCP-backed tool routing through a recording transport instead of direct side effects [@workflow-tests]
- metadata-only redaction across workflow records [@workflow-tests]

The page to read next depends on what you are changing:

- proposal/apply or merge safety: [[workspace-save-workflow]]
- delegated-worker permissions or provider routing: [[assisted-ai-and-delegated-tasks]]
- UI command-center work: [[projection-only-ui-and-desktop]]
