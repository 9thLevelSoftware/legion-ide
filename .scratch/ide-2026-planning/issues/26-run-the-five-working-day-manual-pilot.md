# 26: Run the five-working-day Manual pilot

Status: ready-for-agent

**What to build:** Record real Legion development attempts, fallback and evidence for the declared matrix; pass only with no unresolved data-loss, unauthorized-write or workflow-blocking defects.

**Blocked by:** [06 — Complete selection, Unicode and undo workflows](06-complete-selection-unicode-and-undo-workflows.md); [07 — Complete IME and Vim input workflows](07-complete-ime-and-vim-input-workflows.md); [10 — Complete bounded native canvas arrangement](10-complete-bounded-native-canvas-arrangement.md); [13 — Navigate and replace workspace search results](13-navigate-and-replace-workspace-search-results.md); [14 — Run and stop an interactive terminal task](14-run-and-stop-an-interactive-terminal-task.md); [16 — Review real Rust language edits](16-review-real-rust-language-edits.md); [17 — Run real Rust builds and selected tests](17-run-real-rust-builds-and-selected-tests.md); [18 — Debug a real Rust failure](18-debug-a-real-rust-failure.md); [20 — Resolve a Git conflict and restore history](20-resolve-a-git-conflict-and-restore-history.md); [21 — Recover editor work after abrupt restart](21-recover-editor-work-after-abrupt-restart.md); [22 — Preview and reverse settings/keybinding import](22-preview-and-reverse-settings-keybinding-import.md); [23 — Qualify Windows keyboard and accessibility paths](23-qualify-windows-keyboard-and-accessibility-paths.md); [24 — Qualify Windows large-file responsiveness](24-qualify-windows-large-file-responsiveness.md); [25 — Prove Manual mode does not invoke AI](25-prove-manual-mode-does-not-invoke-ai.md); [11 — Edit a canvas card through editor authority](11-edit-a-canvas-card-through-editor-authority.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 26.

**Traceability:** M1; Q33/Q36. **Verification target:** A01.

## Acceptance criteria

- [ ] Record real Legion development attempts, fallback and evidence for the declared matrix.
- [ ] Pass only with no unresolved data-loss, unauthorized-write or workflow-blocking defects.
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
