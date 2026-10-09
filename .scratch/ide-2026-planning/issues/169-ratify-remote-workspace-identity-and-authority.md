# 169: Ratify remote workspace identity and authority

Status: resolved

**What to build:** Name SSH/container target environments, authentication and file/tool/proposal ownership contracts, reconnect rules and ADR/policy gates; reject ambiguous local/remote authority.

**Blocked by:** [01 — Reconcile the 2026 program delta](01-reconcile-the-2026-program-delta.md); [05 — Preserve edits across save conflicts](05-preserve-edits-across-save-conflicts.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 169.

**Traceability:** M5; S5-05. **Verification target:** A18.

## Acceptance criteria

- [x] Name SSH/container target environments, authentication and file/tool/proposal ownership contracts, reconnect rules and ADR/policy gates.
- [x] Reject ambiguous local/remote authority.
- [x] Every scope, configuration and ownership decision is reviewable and traceable to the approved specification and existing canonical records; unresolved prerequisites are explicitly recorded.
- [x] Preserve existing requirement IDs and approved scope; planning evidence does not promote implementation or product acceptance.

## Verification

- Run `cargo run -p xtask -- docs-hygiene` for documentation changes; use the existing completion/register validators when canonical records change. Record commands and their actual scope.
- Resolve named configurations and ownership against existing records; missing tools, access or ratification stay explicit, without fabricated acceptance.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
- 2026-10-09: Claimed the bounded contract lane on `codex/ide-2026-wave2-remote-authority`, base `ee6896f1e340113cfa1cddbd873f2bc7cf0cb8e7`. Prerequisites 01 and 05 are resolved. Root reviews planning acceptance; Pauli owns the single composed docs-hygiene gate and commits. No remote activation or provisioning is authorized by this document task.
- 2026-10-09: Root independently reviewed the complete contract: PASS for planning only. Pauli verified both frozen handoff hashes and restored the original generic verification requirement. This resolution is gated on the single composed docs-hygiene check; it does not accept S5-05 implementation or A18. Actual target prerequisites remain assigned to tickets 170 onward.
- 2026-10-09: Final composed `cargo run -p xtask -j 2 --target-dir D:/legion-ide-2026-tools/qualification-target -- docs-hygiene` passed, exit 0 (Cargo 0.41 seconds). Log: `D:/legion-ide-2026-notes/wave1-final-docs-hygiene-ddba84fe.log`. Ticket 169 is resolved for planning only; canonical registers are unchanged and no runtime test, host connection or activation was performed.

## Answer

Delivered the [remote workspace authority contract](../../../plans/completion/ide-2026-remote-workspace-authority-contract.md).
It selects separate Ubuntu 24.04 x64 SSH and rootless Docker/OCI container
qualification targets, preserves all four required desktop client platforms and
the separate remote language/project journeys, and binds authentication,
workspace/file/process/proposal identities to explicit target authority.

The contract rejects ambiguous local/remote paths, stale approvals and handles,
unverified host/container replacement, duplicate apply after lost replies, and
false termination or save success. It names source preservation, reconnect,
agent install/rollback/cleanup, extension and forwarding boundaries, with future
ADR/protocol/dependency/policy and real external-effect qualification gates.

**Manual remains zero-egress.** No SSH/container connection, provisioning or
automatic reconnect exception is introduced. This profile requires an explicitly
permitted non-Manual mode without requiring AI inference. Any future Manual
remote exception requires a separate explicit mode/privacy ADR and qualification.

Conflicts/drift are recorded rather than edited elsewhere: S5-05's occupied
ADR-0055 number; planning ticket versus larger protocol package scope and missing
dependency edges; client-only matrix versus target manifests; old fixture-only
evidence versus live services; SSH versus the accepted-equivalent transport gate;
Manual/privacy wording; the SSH scenario's impossible blanket no-network-on-auth-
refusal oracle; retired ADR-0046 references; and unrestricted terminal/bind-mount
writes versus proposal authority.

**Acceptance assessment:** all four planning criteria are satisfied by the
independently reviewed contract. No unresolved owner preference prevents this bounded contract; existing
engineering authorization covers the choices. Actual host/key/user/credential,
engine/image/agent/tool digests, frozen fixtures and observers remain external
prerequisites for 170–211, not fabricated facts or a reason to ask for another
design permission. Root authorized this narrow planning resolution subject to
the composed documentation gate. This does not accept S5-05 implementation or
A18 product qualification.

Worker verification passed for the two files' whitespace and all 25 local link
targets. The cited 18 remote requirement IDs, 15 remote scenario IDs and four
client configuration IDs exist in the unchanged canonical registers. The
new-file `git diff --no-index --check` returned its difference status 1 with no
whitespace errors; the tracked-file `git diff --check` passed. No Cargo tests,
docs-hygiene, provisioning, network or activation ran. Pauli's composed
docs-hygiene gate was pending at worker handoff and subsequently passed as
recorded above. The worker changed only this ticket and the linked contract;
Pauli's integration updates the execution counts separately. The MCP handoff
and all canonical maps/registers remain unchanged by this documentation slice.
