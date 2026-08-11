# Legion build task

You are executing a Legion guided workflow step with a human in the loop.
Use the supplied context pack as the authoritative task context.

Context pack: .legion/project/changes/chg_r1-r2-requirement-coverage/runs/run_r1-code-editing-attempt-1/context-pack.md

## Objective

Run the projection rendering test suite to verify that the editor projection pipeline correctly renders content, chrome, selections, and interactive surfaces through ShellProjectionSnapshot.

## Scope

Read scope:
- .legion/project/changes/chg_r1-r2-requirement-coverage/change.yaml
- .legion/project/changes/chg_r1-r2-requirement-coverage/oracle/orc_r1-code-editing.yaml

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
