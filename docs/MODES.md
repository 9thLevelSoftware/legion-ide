# Legion Product Modes

Legion has four primary modes. Mode policy is a product contract, not a visual preference.

## Manual

Manual mode is deterministic IDE mode.

Allowed:
- editor, tabs, file tree, search, structural search, symbols, problems;
- terminal/debug/test/git panels when permitted by workspace trust;
- local project operations routed through existing app/workspace authorities.

Forbidden:
- AI panels;
- hosted provider routes;
- cloud worker lanes;
- autonomous worker execution;
- hosted telemetry;
- any network-capable AI action.

Completion requirement: tests must prove Manual panel filtering excludes AI/network/cloud/worker surfaces.

## Assist

Assist mode allows human-in-control AI help.

Allowed:
- inline prediction previews;
- assistant right rail with citations;
- explanation and analysis;
- proposal-only edits;
- provider routes that pass policy, privacy, and trust gates.

Forbidden:
- direct file mutation by provider/agent code;
- autonomous task execution;
- hidden provider invocation;
- raw data retention without consent.

Completion requirement: Assist suggestions must be cancellable, dismissible, auditable, and proposal-mediated before mutation.

## Delegate

Delegate mode runs bounded tasks in disposable workers.

Allowed:
- task packets with allowed/forbidden file scopes;
- sandbox/worktree or copy-based execution;
- explicit tool permission requests;
- worker proposals, evidence, and validation results;
- fleet console, task bar, proposal queue, risk monitor, decision feed.

Forbidden:
- direct main workspace mutation by worker code;
- unbounded file access;
- network escalation without policy approval;
- autonomous merge/apply.

Completion requirement: a delegated task can proceed from scoped packet to review-ready proposal with evidence and containment tests.

## Legion Workflows

Legion Workflows mode coordinates multi-step Legion workflows.

Allowed:
- task graphs and dependencies;
- worker assignment and validation lanes;
- workflow builder/runbook/trigger surfaces;
- risk, budget, kill-switch, sign-off, and merge-readiness gates.

Forbidden:
- bypassing validation gates;
- applying proposals without app/user authority;
- autonomous merge before final approval;
- storing raw traces by default.

Completion requirement: workflows must be replayable from metadata/evidence and must stop safely on policy, conflict, validation, or cancellation failures.
