# Legion build task

You are executing a Legion guided workflow step with a human in the loop.
Use the supplied context pack as the authoritative task context.

Context pack: .legion/project/changes/chg_phase-4-navigation-ui-essentials/runs/run_phase-4-navigation-ui-essentials-attempt-1/context-pack.md

## Objective

Implement and verify phase 4: Navigation & UI Essentials.

## Scope

Read scope:
- .legion/project/changes/chg_phase-4-navigation-ui-essentials/change.yaml
- .legion/project/changes/chg_phase-4-navigation-ui-essentials/oracle/orc_phase-4-navigation-ui-essentials.yaml

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
