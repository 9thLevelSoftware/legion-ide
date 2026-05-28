# Phase 8 Wave Checklist

Phase 8 is intentionally sequential. Plugin, collaboration, remote, delegated task, operations, and final acceptance work all touch shared app/UI/desktop/evidence boundaries and must not run concurrently.

| Wave | Plan | Status | Required Prior Evidence |
|------|------|--------|-------------------------|
| 1 | 08-01 GUI Phase 8 Governance And Evidence Gate | Complete | Phase 7 accepted; legacy Phase 8 substrate evidence preserved |
| 2 | 08-02 Plugin Management And Contribution GUI Workflow | Complete | 08-01 result and GUI Phase 8 evidence stub |
| 3 | 08-03 Collaboration Presence And Shared Proposal GUI Workflow | Complete | 08-02 result and plugin GUI evidence |
| 4 | 08-04 Remote Workspace Manager And Remote Status GUI Workflow | Complete | 08-03 result and collaboration GUI evidence |
| 5 | 08-05 Delegated Task Command Center | Planned | 08-04 result and remote GUI evidence |
| 6 | 08-06 GA Release Update Rollback Incident Evidence | Planned | 08-05 result and delegated task command-center evidence |
| 7 | 08-07 Phase 8 GUI Evidence Capture And Acceptance Gate | Planned | 08-01 through 08-06 result files, all GUI Phase 8 evidence artifacts, platform parity proof |

## Execution Rules

- Do not start a wave until every dependency result file exists and has no unresolved `BLOCKED` items.
- Do not edit `plans/evidence/phase-8/`; that directory is accepted legacy runtime substrate evidence.
- Keep `devil-ui` projection-only.
- Keep plugin, collaboration, remote, terminal, proposal, storage, provider, and security authority in app/protocol/runtime layers.
- Keep delegated task command-center behavior approval-gated. Autonomous apply remains unsupported.
- Keep diagnostics and evidence metadata-only.
