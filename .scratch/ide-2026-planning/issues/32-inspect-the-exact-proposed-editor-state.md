# 32: Inspect the exact proposed editor state

Status: ready-for-agent

**What to build:** Capture proposal plus relevant dirty buffers and dependency identity for inspection; block unrepresentable state and widen unknown impact or declare incomplete coverage.

**Blocked by:** [27 — Inspect resolved Rust change impact](27-inspect-resolved-rust-change-impact.md); [05 — Preserve edits across save conflicts](05-preserve-edits-across-save-conflicts.md); [30 — Invalidate impacted facts after dependency changes](30-invalidate-impacted-facts-after-dependency-changes.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 32.

**Traceability:** M2; F2. **Verification target:** A04.

## Acceptance criteria

- [ ] Capture proposal plus relevant dirty buffers and dependency identity for inspection.
- [ ] Block unrepresentable state and widen unknown impact or declare incomplete coverage.
- [ ] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [ ] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [ ] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.

## Verification

- Existing starting seam: `cargo test -p legion-app --test workspace_vfs_integration`. Inspect/select the relevant behavior before editing; this existing suite alone does not establish the new outcome.
- Assert observable results and the failure/denial/recovery conditions named above. Visible product claims require native-input and external-effect evidence; projections, direct dispatch and browser traces do not qualify native IDE input.
- Run planned checks once; rerun only affected failures after supported fixes. After two distinct failed repairs, return evidence and escalate. Separate local checks from real-provider, platform and release qualification.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
