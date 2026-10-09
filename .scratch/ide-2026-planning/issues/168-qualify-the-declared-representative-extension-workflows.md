# 168: Qualify the declared representative extension workflows

Status: ready-for-agent

**What to build:** Collect actual native evidence per named extension/API/runtime configuration and compare the finite coverage report against the ratified required contract; parsing alone earns no runtime credit.

**Blocked by:** [157 — Recover an interrupted extension update](157-recover-an-interrupted-extension-update.md); [159 — Persist and delete extension global storage](159-persist-and-delete-extension-global-storage.md); [160 — Persist and delete extension secret storage](160-persist-and-delete-extension-secret-storage.md); [162 — Run one bounded VS Code Node-host command](162-run-one-bounded-vs-code-node-host-command.md); [163 — Run one bounded VS Code web-worker contribution](163-run-one-bounded-vs-code-web-worker-contribution.md); [164 — Use an isolated extension webview](164-use-an-isolated-extension-webview.md); [166 — Run and interrupt notebook cells](166-run-and-interrupt-notebook-cells.md); [167 — Use a proposal-mediated custom editor](167-use-a-proposal-mediated-custom-editor.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 168.

**Traceability:** M4; S5-15. **Verification target:** A11/A18.

## Acceptance criteria

- [ ] Collect actual native evidence per named extension/API/runtime configuration and compare the finite coverage report against the ratified required contract.
- [ ] Parsing alone earns no runtime credit.
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
