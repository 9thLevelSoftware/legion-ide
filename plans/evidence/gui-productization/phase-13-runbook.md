# Phase 13 Legion Workflow Runbook

## Purpose

This runbook describes how to operate, review, and recover the Phase 13 Legion Workflow orchestration surface.

The runbook is written for maintainers validating the local checkout, reviewing workflow metadata, and deciding whether a proposal-mediated merge can proceed through app-owned authority.

## Operating Markers

- Local workers: isolated delegated-task sandbox
- Provider-backed workers: routed through assisted-AI consent
- Merge readiness: proposal-mediated approval gate
- Autonomous merge: unsupported until approval
- Raw payload retention: disabled by default

## Launch And Verification Commands

Run these commands from the repository root:

```powershell
cargo run -p xtask -- check-deps
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

Useful targeted workflow checks:

```powershell
cargo test -p legion-protocol --test dto_contracts legion_workflow -- --nocapture
cargo test -p legion-agent legion_workflow -- --nocapture
cargo test -p legion-tracker legion_workflow -- --nocapture
cargo test -p legion-memory legion_workflow -- --nocapture
cargo test -p legion-app --test legion_workflow_integration -- --nocapture
cargo test -p legion-ui legion_workflow -- --nocapture
cargo test -p legion-desktop --test legion_workflow_command_center -- --nocapture
```

## Local Worker Flow

Local workflow workers are scheduled by `LegionWorkflowCoordinator`.

The coordinator identifies ready workers from dependency edges, detects same-target conflicts, records blocked workers, and emits proposal-output metadata.

Local worker execution in app composition routes through the existing delegated-task runtime. That runtime uses isolated worktree/sandbox primitives and produces proposal metadata rather than mutating the main workspace.

Reviewers should verify these facts before approving a workflow:

1. Every local worker has nonzero correlation, causality, and event-sequence metadata.
2. Every generated output is linked to proposal metadata or a proposal-preview identifier.
3. The main workspace remains unchanged by worker execution.
4. Dirty editor buffers become readiness blockers.
5. Conflicts are visible as metadata before any approval decision.

## Provider-Backed Worker Flow

Provider-backed workers do not call an external model directly from the coordinator.

They emit route-required metadata with assisted-AI trust references, consent state, provider class, cost label, health label, and redaction hints.

Before any future provider invocation is added, the app/provider layer must prove:

1. Assisted-AI consent allows the requested provider route.
2. Redaction and context manifest metadata are present.
3. Provider health and cost labels are visible.
4. Raw provider payload retention is explicitly governed.
5. The result returns as proposal metadata, not direct workspace mutation.

## Merge Readiness

Merge readiness is a metadata decision, not a git merge operation.

The app evaluates readiness from workflow session state, conflicts, verification gates, sign-off records, approval metadata, rollback evidence, audit metadata, and dirty workspace state.

Readiness must remain blocked when any of these conditions apply:

1. The workspace has dirty buffers.
2. A workflow conflict is unresolved.
3. A verification gate lacks passing evidence.
4. A required sign-off is absent.
5. The approval record is missing or stale.
6. Rollback metadata is absent.
7. Audit-before-success evidence is absent.
8. Any workflow worker has not completed.

Approval allows a proposal-mediated next step only. It does not enable autonomous merge or autonomous apply.

## Verification And Sign-Off

Verification gates should reference deterministic command evidence. The evidence should use command labels, pass/fail outcomes, timestamps, hashes, identifiers, and redaction-safe summaries.

Sign-off records should identify the workflow session, sign-off id, reviewer role, status, correlation id, causality id, and event sequence.

Do not store raw prompts, raw source bodies, raw provider payloads, full terminal output bodies, or raw worker logs in tracker or memory evidence.

## Conflict Recovery

When a conflict appears:

1. Inspect the workflow command-center row for conflict id, affected target, workers, and summary label.
2. Confirm no app-owned proposal has been applied automatically.
3. Resolve the conflict through app-owned workflow APIs or a reviewed proposal path.
4. Re-run the relevant targeted workflow tests.
5. Re-run merge-readiness evaluation.
6. Record resolution metadata in tracker evidence.

If the conflict affects dirty local files, save, reject, or isolate those local edits before re-evaluating readiness.

## Retention Policy

Raw payload retention: disabled by default.

Tracker records may retain metadata identifiers, hashes, labels, redaction hints, route references, conflict summaries, verification summaries, sign-off status, and event ordering metadata.

Memory outcome candidates remain proposed until explicit session-scoped or project-scoped consent is granted.

Denied or absent consent leaves the candidate unretained while preserving enough metadata for review.

## Rollback Notes

Phase 13 does not add an autonomous rollback executor.

Rollback evidence is metadata required for readiness. Actual rollback must use the existing reviewed app/proposal workflow or a maintainer-controlled git operation outside UI/desktop authority.

If a workflow-created proposal is rejected, preserve the tracker evidence and memory consent state so the rejected proposal can be audited without retaining raw payloads.

## Known Limits

- Autonomous merge remains unsupported until approval.
- Autonomous apply remains unsupported.
- Provider-backed workers are route metadata only in this phase.
- Desktop request handling is review/status oriented and does not execute workers.
- UI command intents do not own sessions, text, editor buffers, files, providers, proposals, or merge state.
- The runbook does not replace repository verification gates.
