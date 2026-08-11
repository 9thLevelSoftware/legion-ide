# Legion build task

You are executing a Legion guided workflow step with a human in the loop.
Use the supplied context pack as the authoritative task context.

Context pack: .legion/project/changes/chg_phase-3-live-lsp-integration/runs/run_phase-3-live-lsp-integration-attempt-1/context-pack.md

## Objective

Implement and verify phase 3: Live LSP Integration.

## Scope

Read scope:
- .legion/project/changes/chg_phase-3-live-lsp-integration/change.yaml
- .legion/project/changes/chg_phase-3-live-lsp-integration/oracle/orc_phase-3-live-lsp-integration.yaml

Write scope:
- .

Forbidden scope:
- .git
- node_modules
- .legion/project
- .legion/var/runtime.sqlite

## Harness Rules

- Read before write.
- Evidence before action.
- Keep the diff minimal and scoped to the task contract.
- Verify before report.
- Do not publish, release, or perform unrelated cleanup.

## Required JSON Result

Return only JSON with this shape:
```json
{
  "status": "succeeded | failed | blocked",
  "summary": "short factual summary",
  "filesChanged": ["path"],
  "commandsRun": [{"command": "pnpm", "args": ["test"], "exitCode": 0}],
  "findings": []
}
```
