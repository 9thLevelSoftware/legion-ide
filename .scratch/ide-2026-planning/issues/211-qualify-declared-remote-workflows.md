# 211: Qualify declared remote workflows

Status: ready-for-agent

**What to build:** Observe real SSH/container native journeys for the required target/service matrix and record per-scenario evidence; simulated connection projections cannot count. Join every connector/service/language matrix cell explicitly; a missing remote category stays open.

**Blocked by:** [26 — Run the five-working-day Manual pilot](26-run-the-five-working-day-manual-pilot.md); [182 — Recover SSH disconnects without duplicate apply](182-recover-ssh-disconnects-without-duplicate-apply.md); [183 — Recover container rebuild/reconnect safely](183-recover-container-rebuild-reconnect-safely.md); [175 — Search and navigate a remote workspace](175-search-and-navigate-a-remote-workspace.md); [178 — Run tests and debug a remote program](178-run-tests-and-debug-a-remote-program.md); [177 — Use live remote language assistance](177-use-live-remote-language-assistance.md); [179 — Review and stage an exact remote Git change](179-review-and-stage-an-exact-remote-git-change.md); [180 — Inspect and revoke a remote port forward](180-inspect-and-revoke-a-remote-port-forward.md); [181 — Activate a supported extension remotely](181-activate-a-supported-extension-remotely.md); [184 — Upgrade and remove the remote agent safely](184-upgrade-and-remove-the-remote-agent-safely.md); [185 — Complete container search and navigation](185-complete-container-search-and-navigation.md); [186 — Complete container interactive terminal](186-complete-container-interactive-terminal.md); [187 — Complete container Git review and conflict](187-complete-container-git-review-and-conflict.md); [188 — Complete container port forwarding](188-complete-container-port-forwarding.md); [189 — Complete container extension activation](189-complete-container-extension-activation.md); [190 — Complete TypeScript remote language over SSH](190-complete-typescript-remote-language-over-ssh.md); [191 — Complete TypeScript remote tests over SSH](191-complete-typescript-remote-tests-over-ssh.md); [192 — Complete TypeScript remote debug over SSH](192-complete-typescript-remote-debug-over-ssh.md); [193 — Complete JavaScript remote language over SSH](193-complete-javascript-remote-language-over-ssh.md); [194 — Complete JavaScript remote tests over SSH](194-complete-javascript-remote-tests-over-ssh.md); [195 — Complete JavaScript remote debug over SSH](195-complete-javascript-remote-debug-over-ssh.md); [196 — Complete Python remote language over SSH](196-complete-python-remote-language-over-ssh.md); [197 — Complete Python remote tests over SSH](197-complete-python-remote-tests-over-ssh.md); [198 — Complete Python remote debug over SSH](198-complete-python-remote-debug-over-ssh.md); [199 — Complete Rust remote language over container](199-complete-rust-remote-language-over-container.md); [200 — Complete Rust remote tests over container](200-complete-rust-remote-tests-over-container.md); [201 — Complete Rust remote debug over container](201-complete-rust-remote-debug-over-container.md); [202 — Complete TypeScript remote language over container](202-complete-typescript-remote-language-over-container.md); [203 — Complete TypeScript remote tests over container](203-complete-typescript-remote-tests-over-container.md); [204 — Complete TypeScript remote debug over container](204-complete-typescript-remote-debug-over-container.md); [205 — Complete JavaScript remote language over container](205-complete-javascript-remote-language-over-container.md); [206 — Complete JavaScript remote tests over container](206-complete-javascript-remote-tests-over-container.md); [207 — Complete JavaScript remote debug over container](207-complete-javascript-remote-debug-over-container.md); [208 — Complete Python remote language over container](208-complete-python-remote-language-over-container.md); [209 — Complete Python remote tests over container](209-complete-python-remote-tests-over-container.md); [210 — Complete Python remote debug over container](210-complete-python-remote-debug-over-container.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 211.

**Traceability:** M5; S5-15. **Verification target:** A18.

## Acceptance criteria

- [ ] Observe real SSH/container native journeys for the required target/service matrix and record per-scenario evidence.
- [ ] Simulated connection projections cannot count. Join every connector/service/language matrix cell explicitly.
- [ ] A missing remote category stays open.
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
