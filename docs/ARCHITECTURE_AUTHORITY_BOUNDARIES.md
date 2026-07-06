# Legion Architecture Authority Boundaries

Legion is built around strict ownership boundaries. These boundaries must be preserved when implementing the consolidated e2e plan.

## UI and desktop

The UI/desktop layer is projection-only.

It may:
- render snapshots and projections;
- emit command dispatch intents;
- show mode, trust, proposal, evidence, worker, risk, and validation state;
- collect user decisions such as accept/reject/cancel/configure.

It must not:
- own editor text/session state;
- write files directly;
- run providers or workers directly;
- bypass app/runtime policy.

## App composition

The app layer owns workflow composition and routes requests to authoritative services.

It may:
- coordinate editor, workspace, proposal, provider, agent, tracker, memory, and observability services;
- create proposal workflows;
- enforce mode policy and workspace trust;
- produce projections for UI.

It must not:
- let AI/provider/worker output mutate workspace files without proposal review and authority gates;
- silently retain raw payloads for training;
- treat cloud responses as trusted writes.

## Workspace/project

The workspace/project layer owns filesystem mutation through approved workflows.

It may:
- open/read/save files;
- enforce fingerprints, versions, generations, and conflict checks;
- write approved proposal results.

It must not:
- accept worker/provider direct writes to the main workspace;
- disable stale/conflict checks for convenience;
- fall back to unsafe non-atomic writes.

## AI/provider

Provider code owns model invocation adapters only.

It may:
- route requests according to provider policy;
- return completions, embeddings, inline predictions, route health, and metadata;
- refuse unsupported or denied routes.

It must not:
- invoke hidden network calls in Manual/offline/air-gap mode;
- mutate files;
- store raw prompts or responses unless an explicit consented trace path is used;
- bypass privacy inspector or permission budgets.

## Agent/worker

Worker code owns bounded task execution in disposable lanes.

It may:
- prepare sandbox/worktree/copy lanes;
- execute allowed commands within policy;
- create patch/documentation/analysis/test proposals;
- emit evidence records.

It must not:
- mutate the main workspace directly;
- read forbidden files;
- escalate network/tool permissions without explicit policy decisions;
- merge/apply autonomously.

## Tracker, memory, telemetry, and training traces

Default tracker/memory/telemetry records are metadata-only.

Raw diffs, model outputs, command logs, and validation outputs may enter training traces only when all of the following are true:

1. explicit user/project consent exists;
2. secret scanning and redaction pass;
3. payload hashes and deletion handles are recorded;
4. export is visible and user-controlled;
5. retention policy is enforced.

## Cloud lane

Cloud is an opt-in worker capacity extension, not an authority extension.

Cloud lanes may return status, events, proposals, and evidence. Local app/workspace authorities still own review, validation, and application.

---

## M9 apply-gate activation state (PKT-APPLY, 2026-07-05)

The following apply-path policy gates are now wired and enforced:

### ProposalApplyGate (apply_workspace_proposal)

Every call to `apply_workspace_proposal` passes through a `ProposalApplyGate` check before the payload dispatch:

| Capability namespace | Gate behaviour |
| --- | --- |
| `fs.*` | Allow in Trusted workspaces; Deny in Untrusted/Unknown |
| `plugin.*` | `DenyByDefaultBroker` evaluation — denied by default |
| `remote.*` | `DenyByDefaultBroker` evaluation — denied by default |
| `collaboration.*` | `DenyByDefaultBroker` evaluation — denied by default |
| `terminal.*` | `DenyByDefaultBroker` evaluation — denied by default |
| Everything else (e.g. `editor.write`) | Allowed (lifecycle and workspace checks are sufficient) |

Denial at the gate records a `ProposalLifecycleState::Denied` transition with audit code `proposal.apply_gate_denied` and calls `observe_proposal_response` (persists an audit row).

### TerminalCommand payload denial

`ProposalPayload::TerminalCommand` is unconditionally denied at the apply payload match (`proposal.terminal_command_apply_denied`) as a defense-in-depth measure. The primary enforcement is at validate (Unsupported / unknown terminal target).

### BatchRuntimeApplyPolicy (commit_blocked / finalize_blocked)

`BatchExecutionContract::commit_blocked` and `finalize_blocked` are now derived from `BatchRuntimeApplyPolicy`:
- Default policy: `enabled: false` → `commit_blocked = true`, `finalize_blocked = true` (fail-closed, backward-compatible)
- Trusted workspace + `enabled: true` policy → `commit_blocked = false`, `finalize_blocked = false`
- Untrusted workspace + any policy → always blocked

### LSP rename end-to-end path

`approve_and_apply_rename_proposal(proposal_id)` provides a Previewed→Approved→Applied path for LSP rename proposals. The apply gate and workspace trust checks still apply.
