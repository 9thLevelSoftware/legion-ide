# 60: Qualify the assisted milestone

Status: ready-for-agent

**What to build:** Demonstrate real agents and predictions with contained proposals, cancellation and restart on the declared configuration; missing credentials block only this acceptance, not Manual evaluation.

**Blocked by:** [39 — Qualify the first complete change-review journey](39-qualify-the-first-complete-change-review-journey.md); [52 — Qualify the named Codex ACP integration](52-qualify-the-named-codex-acp-integration.md); [53 — Qualify the named Claude Code ACP integration](53-qualify-the-named-claude-code-acp-integration.md); [49 — Use and qualify real next-edit predictions](49-use-and-qualify-real-next-edit-predictions.md); [55 — Reconcile uncertain actions before explicit resume](55-reconcile-uncertain-actions-before-explicit-resume.md); [58 — Review expanded skill permissions](58-review-expanded-skill-permissions.md); [59 — Enforce and reconcile an activation budget](59-enforce-and-reconcile-an-activation-budget.md); [45 — Remove obsolete provider cancellation paths](45-remove-obsolete-provider-cancellation-paths.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 60.

**Traceability:** M3; S3-07/S4-06. **Verification target:** A07–A12.

## Acceptance criteria

- [ ] Demonstrate real agents and predictions with contained proposals, cancellation and restart on the declared configuration.
- [ ] Missing credentials block only this acceptance, not Manual evaluation.
- [ ] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [ ] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [ ] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.

## Verification

- Existing starting seam: `cargo test -p xtask --test completion_evidence`. Inspect/select the relevant behavior before editing; this existing suite alone does not establish the new outcome.
- Assert observable results and the failure/denial/recovery conditions named above. Visible product claims require native-input and external-effect evidence; projections, direct dispatch and browser traces do not qualify native IDE input.
- Run planned checks once; rerun only affected failures after supported fixes. After two distinct failed repairs, return evidence and escalate. Separate local checks from real-provider, platform and release qualification.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
