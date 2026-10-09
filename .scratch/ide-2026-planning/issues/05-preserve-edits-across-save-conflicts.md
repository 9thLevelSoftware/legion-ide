# 05: Preserve edits across save conflicts

Status: resolved

**What to build:** Open, edit, encounter an external overwrite and recover without losing dirty text; exercise current proposal/version guards and repair only the demonstrated gaps.

**Blocked by:** [02 — Ratify the Windows/Rust pilot configuration](02-ratify-the-windows-rust-pilot-configuration.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 05.

**Traceability:** M1; S1-04; work preservation. **Verification target:** A02.

## Acceptance criteria

- [x] Open, edit, encounter an external overwrite and recover without losing dirty text.
- [x] Exercise current proposal/version guards and repair only the demonstrated gaps.
- [x] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [x] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [x] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.

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

- 2026-10-09: Pauli independent review PASS for the frozen three-file patch. Worker commit `9b3db4b7ba474065b3952d513b181ef722e2467b` integrated as `9cff36682cae2dd8b1c17bd8e8e5bc566594b340`. Reviewed buffer/path identity, retained metadata, acknowledged-save and fresh-open behavior, hot-exit restoration, actual disk oracles and receipt attribution. No successful worker checks repeated.

## Answer

Accepted for ticket 05's complete external-overwrite preservation/recovery slice
at the specification's **A02 app workflow plus storage-reopen boundary**. The
[reviewed receipt](../../../plans/evidence/ide-2026-ticket005-save-conflict-recovery.md)
records 15 unique focused passing tests, formatting, dependency policy,
documentation hygiene and whitespace results. The retained-buffer baseline
regression had a behavioral red; the recovery test's initial event-name mismatch
was a corrected test assumption, not another product defect.

The observable outcome preserves dirty Unicode/CRLF text and external original
bytes through refused save, dirty-close cancellation, real storage failure and
actual store reopen in a fresh app. Reactivation preserves the old save baseline;
explicit new-file/edit/save operations create a separate recovery copy through
existing proposal authority, and another fresh app verifies its exact saved text.

This resolves this ticket only. It does not claim native GUI input, a native
Save As/recovery control, abrupt-process-kill recovery, all A02 dirty-dependency
or migration scenarios, ticket 04, ticket 21, or the Manual pilot. Canonical
product-readiness records receive no broader acceptance promotion. Combined-wave
compilation and documentation gates remain scheduled separately.
