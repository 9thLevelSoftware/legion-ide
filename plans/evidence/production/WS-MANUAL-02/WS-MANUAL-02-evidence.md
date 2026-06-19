# WS-MANUAL-02 Large Files and Workspace Scale Evidence

## Workstream status

- Status: In Progress
- Plan: `docs/superpowers/plans/2026-06-19-ws-manual-02-large-files-workspace-scale.md`
- Master plan reference: `plans/legion-production-master-plan-v0.2.md` WS-MANUAL-02 (lines 308-337)

## Product gate

- `PR-UI-002` large workspace behavior: Substrate validated -> pending product-workflow evidence

## Evidence records

| Task | Description | Status | Evidence |
| --- | --- | --- | --- |
| SCALE.01 | Reference workspaces defined | Pending | `reference-workspaces.md` |
| SCALE.02 | 100MB measured non-green test | Pending | integration test |
| SCALE.03 | Streaming text viewport for 100MB | Pending | integration test |
| SCALE.04 | Binary file detection and preview refusal | Pending | unit test |
| SCALE.05 | File-size policy projection and UX | Pending | protocol + UI |
| SCALE.06 | Workspace tree open non-blocking | Pending | integration test |
| SCALE.07 | Watcher burst/debounce under churn | Pending | integration test |
| SCALE.08 | Search cancellation resource cleanup | Pending | integration test |
| SCALE.09 | Memory ceiling measurement | Pending | perf harness |
| SCALE.10 | Stale snapshot/lease tests for large files | Pending | integration test |
