# Manual IDE and Core Language Workflows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Manual mode a dependable native IDE, then qualify complete Rust, TypeScript/JavaScript, and Python development workflows through the shipped desktop application.

**Architecture:** Preserve the authoritative path `native input -> desktop/UI intent -> AppComposition -> authoritative service -> external effect -> app snapshot -> rendered feedback`. `legion-ui` stays projection-only; editor text remains app/editor-owned and every workspace write, including replace, rename, local-history restore, and language edit, remains proposal-mediated. Stage 1 closes ordinary Manual workflows before Stage 2 extends the same authority path to each supported language toolchain.

**Tech Stack:** Rust workspace; eframe/egui native desktop; `legion-app`, `legion-editor`, `legion-text`, `legion-project`, `legion-terminal`, `legion-debug`, `legion-lsp`; Git; Cargo, Node/TypeScript, Python; LSP and DAP.

**Spec:** `docs/superpowers/specs/2026-09-04-product-completion-design.md`

## Global Constraints

- Keep the UI projection-only; do not move buffer/session ownership into `legion-ui`.
- Preserve proposal-mediated saves and the expected fingerprint, file-content version, workspace generation, buffer version, snapshot ID, correlation ID, and causality requirements.
- Preserve default-deny capability policy, metadata-only observability, non-zero correlation/causality/event sequence, explicit egress consent, and Manual mode zero egress.
- Do not treat headless direct `DesktopAction`, mocked LSP/DAP, replayed fixtures, or UI projection assertions as layer-3 product acceptance.
- Make Windows, macOS, and Linux native input, accessibility/focus, DPI, window restore, recovery, and responsive workloads first-class required matrix entries; record a blocker rather than substituting a fixture.
- Do not alter dependency policy/protocol symbols without updating `plans/dependency-policy.md`, `xtask check-deps`, required ADR work, and contract tests together.
- Preserve user work on cancel, terminal/debug process loss, LSP failure, storage failure, external edit, save conflict, restart, and migration failure; show the real failed state.
- Every package below records a `COMP-*` mapping, immutable build/evidence ID, implementation state, acceptance state, configuration coverage, reviewer, and unresolved defect in the Stage-0 completion register before it is accepted.
- Pin candidate identity through the master `candidate.json.code_sha`. Pre-commit checks are supporting evidence only; accepted native runs occur after the implementation is committed and built from that exact candidate SHA. Create a separate evidence-only commit after the run. Never compare an evidence checkout against its current `HEAD`; compare its recorded code SHA to the candidate identity.

---

## Files and boundaries to inspect before implementation

| Path | Responsibility in these stages |
| --- | --- |
| `crates/legion-desktop/src/workflow.rs` | `DesktopEframeApp` translates eframe events and rendered controls into `DesktopAction`; it is the native-input boundary. |
| `crates/legion-desktop/src/bridge.rs` | Defines `DesktopAction` and translates it to `legion_ui::CommandDispatchIntent`. |
| `crates/legion-desktop/src/view.rs` and `src/view/*.rs` | Renders projections, including canvas, terminal, source control, debug and interactive fields. |
| `crates/legion-desktop/src/windowed_e2e.rs` | Current GAP-01 windowed harness; it opens a real window but directly calls `InsertText` and `SaveActive`, so it is not native-input proof. |
| `crates/legion-desktop/tests/common/mod.rs` | Existing `TempWorkspace`, `full_frame_input`, `click_at`, `clickable_center`, and key-input test helpers. |
| `crates/legion-desktop/tests/{canvas_workspace,save_row_2,input_conformance,focus_smoke,ime_smoke,terminal_reachability,source_control_reachability,debug_reachability}.rs` | Existing rendered/headless coverage to retain while strengthening real external-effect checks. |
| `crates/legion-app/src/lib.rs` | `AppComposition` owns command dispatch, save, search/replace, local history, Git, terminal, test explorer and language lifecycle. |
| `crates/legion-app/src/{search.rs,terminal_policy.rs,debug_workflow.rs,language/app_lsp.rs,language/session.rs,language/translate.rs,test_explorer.rs}` | Existing authoritative workflows to extend rather than bypass. |
| `crates/legion-project/tests/{git_workflow,search_workspace,debug_locator}.rs`, `crates/legion-app/tests/{workspace_vfs_integration,terminal_workflow,local_history_workflow,language_tooling_workflow,debug_workflow}.rs` | Lower-layer external-effect and recovery test patterns. |
| `crates/legion-lsp/{src,tests/registry_contract.rs}` | Registry/adapters already name Rust, TypeScript, JavaScript and Python candidates; Stage 2 must turn those declarations into supported, live contracts. |
| `crates/legion-debug/{src,tests/system_adapter_launch_step_dogfood.rs}` | DAP authority and existing real-adapter evidence. |

## Stage 1 — Manual mode: real daily IDE workflow

### S1-01: Populate Manual requirements and scenarios in the Stage-0 register

**Dependencies:** Stage 0 completion register and configuration-matrix package.
**Owner model:** Terra; Astra reviews the matrix and any scope decision.
**Deliverable:** `COMP-MAN-*` rows and scenario records in the authoritative Stage-0 completion register. They name supported OS/architecture, reference hardware, assistive technology, shell, Git version, representative repositories, Manual user journeys and negative/recovery cases. They tag every existing test as component, integrated, or product acceptance and reject fixture-only evidence for the latter.

**Files:**
- Create: `plans/acceptance/manual-scenarios-v1.md`
- Modify: `plans/completion/requirements.json` (canonical Stage-0 register; schema supplied by Stage 0)
- Modify: `docs/E2E_TESTING_CATALOG.md`
- Test: the Stage-0 completion-register validator and its fixtures

**Interfaces:** Consume the Stage-0 completion-register schema, matrix schema, `COMP-*` IDs and validator command. Do not redefine those schemas or introduce a competing status source. Produce Manual rows whose scenario IDs and evidence fields satisfy the Stage-0 validator.

- [ ] Add the Manual rows and scenario records to the Stage-0 schema; include unmapped historical tests as supporting evidence, never as acceptance evidence by name alone.
- [ ] Run the Stage-0 validator with the Manual rows; expected result: non-zero exit that names the missing field and `COMP-*` ID until each required scenario/external oracle/recovery/reviewer field exists.
- [ ] Correct the Manual rows until the Stage-0 validator accepts them or reports an explicit currently blocked configuration; do not change validator rules here.
- [ ] Treat the unavailable historical inputs `docs/UAT_E2E_FINDINGS.md`, `docs/QA_E2E_FINDINGS.md`, and `docs/UX_UI_E2E_FINDINGS.md` as explicitly unavailable (all three are absent in this checkout); do not require reading them or assert that they were reviewed. Establish the baseline from current tests, retained evidence, and the approved product-completion spec. Commit only the scenario/evidence mapping.

**Native acceptance:** On each declared OS, an independent reviewer opens a disposable representative repository, follows the published Manual scenario, and records build ID, interaction capture, external artifacts, failures and recovery result. External oracles include files read from disk, `git status`/`git log`, a child-process exit code, and restart-restored state. A screen that looks correct or an app status label alone fails.

### S1-02: Specify and implement the native input acceptance boundary

**Dependencies:** S1-01.
**Owner model:** Sol engineer for the bounded design and implementation; Sol reviewer before merge.
**Deliverable:** A narrowly scoped design and implementation for an OS-driven native acceptance driver that injects keyboard, pointer, clipboard and IME input into the packaged `eframe::run_native` app and obtains only redacted, externally observable reports. It replaces no product input path and cannot invoke `DesktopRuntime::handle_action` for the behavior being certified.

**Files:**
- Create: the next unused ADR under `plans/adrs/` for the native-product-input harness; allocate the number and record the exact path in the completion-program decisions record before implementation
- Create: `crates/legion-desktop/src/native_acceptance.rs`
- Modify: `crates/legion-desktop/src/lib.rs`
- Modify: `xtask/src/main.rs`
- Create: `xtask/tests/native_product_acceptance.rs`

**Interfaces:** The ADR must define driver process boundary, per-OS provider, focus/readiness protocol, capture/redaction format, timeouts, exit/report semantics, and failure classification. The new native-acceptance scenario runner must not invoke `DesktopRuntime::handle_action` or test-only runtime-enablement calls for behavior it certifies. Keep `windowed_e2e.rs` unchanged as a separately classified integrated/windowed smoke: its direct actions are useful lower-layer coverage, not native-input acceptance. The implementation exposes a single `xtask native-product-acceptance --scenario <id> --report <path>` orchestration entry after the ADR is accepted.

- [ ] Write the unavailable-controller behavior test before selecting a driver. This new acceptance command must produce an explicit `blocked` report, not a green result or a hidden skip, when its declared OS input provider is unavailable:

```rust
#[test]
fn unavailable_input_controller_is_blocked_not_passed() {
    let report = std::env::temp_dir().join(format!(
        "legion-native-acceptance-{}-report.json",
        std::process::id()
    ));
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["native-product-acceptance", "--scenario", "manual-open-type-save", "--report"])
        .arg(&report)
        .env("LEGION_NATIVE_INPUT_DRIVER", "unavailable")
        .status()
        .expect("run native acceptance command");
    let evidence: serde_json::Value = serde_json::from_reader(
        std::fs::File::open(&report).expect("report exists even when blocked"),
    )
    .expect("canonical EvidenceRun JSON");
    assert!(!status.success());
    assert_eq!(evidence["result"], "blocked");
    let _ = std::fs::remove_file(report);
}
```

- [ ] Run `cargo test -p xtask --test native_product_acceptance unavailable_input_controller_is_blocked_not_passed`; expected result before implementation: FAIL because the command/report contract does not exist.
- [ ] Produce the ADR with the exact provider selection and cross-platform implementation seam; have its reviewer accept it before changing `workflow.rs` or adding dependencies.
- [ ] Implement one representative native scenario: focus editor, type an `egui::Event::Text` equivalent through the OS driver, invoke the actual Save command, then verify the saved disk marker outside the app. Keep existing headless tests as lower-layer regression tests.
- [ ] Run the new xtask test plus `cargo run -p xtask -- native-product-acceptance --scenario manual-open-type-save --report target/native-acceptance/manual-open-type-save.json`; expected: canonical `EvidenceRun` JSON is passed only with window/focus/input/save/external-file oracle all present, and is blocked if the OS driver cannot run. Legacy windowed TOML remains an attachment, never the canonical result.
- [ ] Review input security (no arbitrary host input outside the launched window), report redaction, focus loss, IME commit, clipboard denial, and timeout cleanup. Commit ADR and code separately after review.

**Native acceptance:** Package the current candidate, launch it through the harness, click Explorer file, type Unicode and an IME committed string into the focused canvas, use the shipped shortcut/menu Save, terminate and collect report. Read the file with the host filesystem and fail for lost focus, direct-dispatch evidence, missing composition commit, wrong file, or no window.

### S1-03: Complete the existing read-only canvas arrangement and workbench layout contract

**Dependencies:** S1-01, S1-02.
**Owner model:** Terra; Sol review if canvas state/persistence authority changes.
**Deliverable:** A Manual workbench contract covering tab opening/closing, splits, the currently authorized read-only canvas arrangement, draggable cards, pan/zoom, person-drawn connections, multiple windows, layout restore, focus order and keyboard navigation. Preserve ADR-0051: cards remain read-only views of app-owned buffers; keyboard editor input cannot mutate a buffer while the Canvas centre surface is shown. Existing `canvas_workspace` behavior is retained only where its scenario is a real user outcome.

**Files:**
- Modify: `crates/legion-desktop/src/view/canvas_workspace.rs`
- Modify: `crates/legion-desktop/src/view.rs`
- Modify: `crates/legion-desktop/src/workflow.rs`
- Modify: `crates/legion-desktop/src/bridge.rs`
- Modify: `crates/legion-app/src/lib.rs`
- Test: `crates/legion-desktop/tests/canvas_workspace.rs`
- Test: `crates/legion-desktop/tests/native_acceptance_boundary.rs`

**Interfaces:** Consume `DesktopAction` and snapshot projections; produce only existing/approved `CommandDispatchIntent` additions. Canvas cards/ports use ADR-0051 actions (`MoveCanvasNode`, `PlaceCanvasNodes`, `ConnectCanvasNodes`, `DisconnectCanvasNodes`) and remain adapter-local arrangement state. If a layout command requires new persisted data, first add its protocol/storage record and migration in the task, with an ADR if ownership crosses the existing app/storage boundary.

- [ ] Write failing rendered tests for tab close with dirty prompt, split focus transfer, card drag/connection persistence, restoring a two-window layout after restart, and typing on Canvas not mutating the buffer; assert snapshot state and real persisted record, not labels.
- [ ] Implement through projection/action/app ownership; do not store editor text, unsaved edits, or session truth in the view.
- [ ] Run `cargo test -p legion-desktop --test canvas_workspace` and relevant `legion-app` persistence tests; expected: all pass and a corrupt layout produces recoverable defaults plus a visible diagnostic.
- [ ] Run native scenario `manual-workbench-layout-restart`: create split/card arrangement, edit two files, close/restart, restore layout and dirty-recovery prompts; external oracle reads persisted layout metadata and both file states. Record the result after an independent desktop diff review, then create the separate evidence-only commit.

### S1-03B: Bound and approve any expansion beyond the read-only canvas arrangement

**Dependencies:** S1-03.
**Owner model:** Astra for product/authority decision; Sol engineer authors the technical option analysis.
**Deliverable:** An accepted, finite follow-on canvas specification that inventories `docs/ui/canvas-workspace-direction.md`, separates required full-vision outcomes from non-commitment direction, and defines authority/provenance/staleness/performance/accessibility for each proposed derived edge, editable card, or new canvas capability. This is a design gate, not permission to implement the surface.

**Files:**
- Create: `docs/superpowers/specs/2026-09-04-canvas-full-vision-spec.md`
- Modify: `plans/adrs/ADR-0051-canvas-workspace-surface.md` only if the accepted design changes its current contract
- Modify: `plans/dependency-policy.md` only if the accepted design adds a dependency/protocol obligation
- Test: `crates/legion-desktop/tests/canvas_workspace.rs` (preservation tests only in this design package)

**Interfaces:** The spec must enumerate proposed commands/data sources and declare whether each remains adapter-local or needs an app/project-owned authoritative service. It must name how a derived edge is labeled, invalidated and inspected; it cannot infer architecture from an index/LSP/Cargo source without an accepted provenance contract.

- [ ] Inventory every canvas-direction promise and map it to a `COMP-CANVAS-*` requirement, current ADR-0051 permission, proposed owner, acceptance outcome and risk.
- [ ] Write mutually exclusive options for read-only arrangement expansion, editable canvas cards, and derived semantic edges, including data authority, stale-result policy, failure/recovery and performance budget.
- [ ] Have Astra accept one bounded option and required ADR/dependency changes before creating an implementation package.
- [ ] Run `cargo test -p legion-desktop --test canvas_workspace`; expected: the current read-only/no-derived-edge constraints remain green. Commit spec/ADR only after review.

### S1-03C: Implement and qualify every Stage-1 `COMP-CANVAS-*` outcome accepted by the canvas contract

**Dependencies:** S1-03B and its accepted ADR/dependency decisions.
**Owner model:** Sol engineer for cross-authority implementation; Sol reviewer. The product owner alone may alter full-vision scope in a recorded decision; no worker may drop, defer, or reinterpret a required `COMP-CANVAS-*` row to close this package.
**Deliverable:** The complete Stage-1 canvas arrangement, navigation and editing outcomes assigned by S1-03B work through the accepted authority path, have accessible/discoverable controls, preserve current ADR-0051 behavior until the amendment takes effect, and have a native acceptance record per required configuration.

**Files:**
- Modify: `crates/legion-desktop/src/{view/canvas_workspace.rs,view.rs,bridge.rs,workflow.rs}`
- Modify: `crates/legion-app/src/lib.rs` and only the authoritative service paths named in the accepted canvas ADR
- Modify: `crates/legion-editor/src/lib.rs` only if the accepted contract authorizes an editor-owned canvas edit interaction
- Modify: `plans/completion/requirements.json`
- Test: `crates/legion-desktop/tests/canvas_workspace.rs`
- Test: `xtask/tests/native_product_acceptance.rs`

**Interfaces:** Consume the accepted S1-03B command/data/provenance/staleness contract. Produce only its named `DesktopAction`, `CommandDispatchIntent`, app service and projection interfaces. An editable canvas interaction must call the same app/editor edit and project save/proposal routes as the ordinary editor; a derived edge must publish its source, freshness and invalidation status.

- [ ] For each assigned `COMP-CANVAS-*` row, write a failing rendered regression test and a native-scenario expectation that identifies the user control, authoritative effect, external oracle and required recovery case.
- [ ] Implement the accepted commands and projections in bounded commits; retain the ADR-0051 preservation tests until the amendment's replacement behavior is implemented and reviewed.
- [ ] Run `cargo test -p legion-desktop --test canvas_workspace` and the affected app/editor tests named by the accepted contract; expected: actual buffer mutation, saved file, navigation result or provenance-labeled edge state is asserted, not a display label.
- [ ] Run each required packaged native canvas scenario through `xtask native-product-acceptance`; verify real edit/save/restore through host file reads, keyboard and accessibility operation, stale derived-edge invalidation where applicable, focus loss/cancel/restart recovery, and canonical `EvidenceRun` JSON. Commit implementation and evidence/register changes separately after independent review.

### S1-04: Finish editing semantics, input methods, keymaps and settings

**Dependencies:** S1-02, S1-03.
**Owner model:** Terra.
**Deliverable:** Ordinary editing is discoverable and reliable for selections, Unicode/graphemes, undo/redo, multi-cursor, indentation, wrapping, clipboard, IME, Vim, keymap customization, settings profiles and reset.

**Files:**
- Modify: `crates/legion-desktop/src/workflow.rs`
- Modify: `crates/legion-desktop/src/view/interactive_fields.rs`
- Modify: `crates/legion-desktop/src/bridge.rs`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `crates/legion-editor/src/{lib.rs,multi_cursor.rs}` as required by the failing editing test
- Test: `crates/legion-desktop/tests/{input_conformance,ime_smoke,focus_smoke,keyboard_nav,canvas_workspace}.rs`
- Test: `crates/legion-desktop/tests/native_acceptance_boundary.rs`

**Interfaces:** Reuse `DesktopAction::{InsertText,ReplaceRange,DeleteRange,ClipboardPaste,ImeCommit,Undo,Redo,SetCursor,SetSelection,AddCursorAbove,AddCursorBelow,ClearExtraCursors}` and `AppComposition` editing methods. Do not invent a second input model.

- [ ] Add focused failing tests for a Unicode selection/edit/undo chain, multi-cursor insertion, an IME commit after focus change, conflicting keymap binding rejection, and settings reset after restart.
- [ ] Implement only missing translation, focus and persistence links; keep `VimState` pure and route product Vim through `AppComposition::dispatch_vim_key`.
- [ ] Run `cargo test -p legion-desktop --test input_conformance --test ime_smoke --test keyboard_nav` plus the editor test named by the changed module; expected: pass.
- [ ] Native scenario `manual-editing-input-recovery`: use keyboard, pointer, clipboard, IME and Vim in one file, intentionally lose/regain focus, restart, then verify text/undo state/settings or the documented recovery outcome. Record UIA/AX/AT-SPI observations per platform, then create the separate evidence-only commit after review.

### S1-05: File operations, navigation, search and reviewable replacement

**Dependencies:** S1-01, S1-03, S1-04.
**Owner model:** Terra; Sol review for cross-workspace or write-authority changes.
**Deliverable:** Explorer create/rename/move/delete/open/reveal, quick file/symbol/definition/references navigation, literal/regex workspace search, replacement preview/apply/cancel and stale-result handling work on real trees while preserving ignores and write safeguards.

**Files:**
- Modify: `crates/legion-desktop/src/view.rs`
- Modify: `crates/legion-desktop/src/bridge.rs`
- Modify: `crates/legion-app/src/{lib.rs,search.rs}`
- Modify: `crates/legion-project/src/lib.rs`
- Test: `crates/legion-project/tests/search_workspace.rs`
- Test: `crates/legion-app/tests/workspace_replace_proposal.rs`
- Test: `crates/legion-desktop/tests/canvas_workspace.rs`

**Interfaces:** Reuse `WorkspaceSearchReplaceCapability`, `AppComposition` proposal routes, canonical paths and file identities. A replacement must arrive as a proposal and preserve stale snapshot/conflict denial.

- [ ] Write failing tests that assert real filesystem results after UI-created/renamed files, a regex replace preview that is cancelled without disk mutation, a stale search result rejected after an external edit, and an ignored file absent from search results.
- [ ] Implement missing UI actions and authoritative routes; reject invalid/cross-root paths before any mutation.
- [ ] Run `cargo test -p legion-project --test search_workspace`, `cargo test -p legion-app --test workspace_replace_proposal`, and targeted desktop test; expected: pass.
- [ ] Native scenario `manual-refactor-search-review`: navigate multi-file symbol, run regex replace, inspect diff, cancel one preview, apply another, trigger an external edit before apply, and verify accepted/rejected disk state with host reads. Commit following source-control review.

### S1-06: Make terminal execution and interactive TUI behavior real

**Dependencies:** S1-02, S1-04.
**Owner model:** Sol engineer because PTY lifecycle and cross-platform input are concurrency/process-sensitive; Sol reviewer.
**Deliverable:** A trusted-workspace terminal that starts in workspace root, executes user/UI commands rather than only displaying labels, supports control keys, resize, scrollback/search/output rendering, interactive TUI use, working-directory changes, cancellation and child cleanup. Its canonical acceptance evidence carries structured child PID, launch timestamp, command class, terminal exit status and independently observed effect; transcript error text is supporting diagnostic evidence only.

**Files:**
- Modify: `crates/legion-app/src/{lib.rs,terminal_policy.rs}`
- Modify: `crates/legion-terminal/src/{session.rs,grid.rs,vt100.rs,conpty.rs}` as required by traced defect
- Modify: `crates/legion-desktop/src/{workflow.rs,view/terminal_panel.rs}`
- Test: `crates/legion-app/tests/terminal_workflow.rs`
- Test: `crates/legion-terminal/tests/{terminal_grid,platform_shell_smoke,conpty_parity}.rs`
- Test: `crates/legion-desktop/tests/terminal_reachability.rs`

**Interfaces:** Preserve `DesktopAction::{TerminalLaunch,TerminalInput,TerminalOutputPoll}` and terminal policy authorization. `TerminalLaunch { command_label }` remains a label/audit request; callers that promise command execution must explicitly send `TerminalInput` or use an approved command-execution API designed in this package.

- [ ] Strengthen the weak rendered Tests assertion with a real process-only oracle. Replace `ran = transcript.contains("cargo test")` with the Cargo error output for the intentionally manifest-less fixture:

```rust
let no_manifest = if cfg!(windows) {
    "could not find `Cargo.toml`"
} else {
    "could not find `Cargo.toml`"
};
ran = transcript.contains(no_manifest);
```

- [ ] Run `cargo test -p legion-desktop --test terminal_reachability clicking_run_cargo_test_sends_the_command_to_the_terminal`; record the observed baseline. A pass earns integrated credit only; a failure becomes a traced repair. Add a negative control that launches the shell without `TerminalInput` and proves it cannot satisfy the execution oracle.
- [ ] Repair only an observed execution/status mismatch, then add tests for resize, Ctrl-C interruption, child cleanup and shell cwd. Do not make terminal launch auto-run an arbitrary label.
- [ ] Run `cargo test -p legion-app --test terminal_workflow`, `cargo test -p legion-terminal --test platform_shell_smoke`, and the targeted desktop test; expected: pass.
- [ ] Native scenario `manual-terminal-tui`: open terminal from the actual panel, run a manifest-less `cargo test`, record its structured PID and non-zero exit, and independently observe both the Cargo diagnostic and process completion; then run a declared minimal interactive TUI fixture, resize it, send control key/cancel, close app, and prove child cleanup by PID/process check. Record per-OS shell and failures in canonical `EvidenceRun` JSON; create the separate evidence-only commit after process-lifecycle review.

### S1-07: Complete Git, review surface and local history recovery

**Dependencies:** S1-03, S1-05.
**Owner model:** Terra; Sol review for conflict and persistence changes.
**Deliverable:** Visible and usable status/diff/stage/unstage/commit/branch/conflict/remote verbs, reviewable changes, local history browsing/restoration, dirty recovery, external-change conflict, checkpoint and storage-failure recovery.

**Files:**
- Modify: `crates/legion-desktop/src/view/source_control.rs`
- Modify: `crates/legion-desktop/src/{bridge.rs,workflow.rs}`
- Modify: `crates/legion-app/src/{lib.rs,git_inspection.rs,git_remote.rs,git_policy.rs}`
- Modify: `crates/legion-storage/src/local_history.rs`
- Test: `crates/legion-desktop/tests/source_control_reachability.rs`
- Test: `crates/legion-app/tests/{git_workflow,git_remote_policy_workflow,local_history_workflow,workspace_vfs_integration}.rs`

**Interfaces:** Reuse local history methods exposed from `AppComposition` and Git policy; remote commands remain policy/credential mediated. Restores use a proposal and retain stale/conflict rejection.

- [ ] Write failing tests with real repositories for staged versus unstaged diff, branch switch blocked by dirty edit, merge conflict choice, rejected unauthorized remote, local-history restore proposal, and external overwrite between open/save.
- [ ] Implement missing visible paths and error details; do not equate a projection row with a completed Git operation.
- [ ] Run the named test binaries and `cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict`; expected: pass.
- [ ] Native scenario `manual-git-history-recovery`: edit/stage/commit, make a conflicting branch merge, resolve and commit, restore a prior local-history version via proposal, trigger external overwrite, restart during a dirty buffer, then verify `git log`, index, worktree and disk contents outside Legion. Create the separate evidence-only commit after independent review.

### S1-08: Close Manual accessibility, window lifecycle, large-file and workload evidence

**Dependencies:** S1-02 through S1-07.
**Owner model:** Sol engineer for platform integration; Terra may prepare fixtures/evidence.
**Deliverable:** Native accessibility trees and keyboard-only operations, DPI/window restore/focus correctness, large-file streaming behavior, and measured product workloads on all supported OSes.

**Files:**
- Modify: `crates/legion-desktop/src/{workflow.rs,view.rs}` only after accessibility trace
- Modify: `xtask/src/main.rs`
- Modify: `plans/product-readiness-ledger.md` only as a derived reference to canonical `EvidenceRun`/requirements records; never as a second mutable acceptance status
- Create: `plans/evidence/production/manual-stage-1/README.md`
- Test: existing targeted desktop accessibility/input tests plus selected performance harness tests

**Interfaces:** Consume the matrix in S1-01; produce redacted, immutable native evidence records with actual accessibility provider and measured workload values. Avoid adding a test-only semantic tree.

- [ ] Run current native and accessibility evidence paths to establish a failing/blocked baseline; record whether Windows UIA/Narrator, macOS AX/VoiceOver and Linux AT-SPI/Orca observe the real window.
- [ ] Implement only observed platform defects; add a focused regression test and real native rerun for each defect.
- [ ] Run `cargo run -p xtask -- perf-harness`, `cargo run -p xtask -- verify-perf-harness`, the native acceptance scenarios and `cargo run -p xtask -- windowed-gui-e2e`; expected: no synthetic result is presented as renderer-backed proof.
- [ ] Acceptance: keyboard-only complete S1 scenario, inspect live accessibility descendants/actions, restore maximized and split windows after restart, open/edit/save declared large file, and capture startup/paint/memory evidence. Any unavailable AT produces a blocked matrix row, not a pass. Commit evidence mapping only after independent review.

### Stage 1 exit checklist

- [ ] Every `COMP-MAN-*` requirement and required OS/configuration has a passing, current layer-3 scenario. A blocker remains on the critical path and prevents the Stage 1 exit; it is recorded rather than converted into a pass.
- [ ] The packaged native application, not direct `DesktopAction`, completes the open/edit/save, workbench, navigation/replace, terminal/TUI, Git/history/recovery and accessibility journeys.
- [ ] External oracles prove files, Git state, process result/cleanup, persistence and recovery; no status label is the sole oracle.
- [ ] Independent reviewer approves the evidence; no unresolved P0/P1 Manual defect remains.

## Stage 2 — Rust, TypeScript/JavaScript and Python complete workflows

### S2-01: Ratify the three-language support contract and representative projects

**Dependencies:** S1-01; S1 exit for shared Manual prerequisites.
**Owner model:** Sol engineer for toolchain/adapter contract; Astra accepts scope.
**Deliverable:** Core-language entries in the canonical Stage-0 compatibility matrix (`plans/completion/matrix.json`) that name each language's projects, package/environment discovery, LSP server, formatter, test runner, build/task runner, debug adapter, supported operating systems and versions. It identifies existing Rust-only assumptions and states the approved, bounded replacements. Any readable language-specific matrix is an immutable evidence attachment derived from this canonical record.

**Files:**
- Create: `plans/acceptance/fixtures/{rust,typescript-javascript,python}/README.md`
- Modify: `plans/completion/matrix.json`
- Modify: `crates/legion-lsp/tests/registry_contract.rs`
- Modify: `plans/product-readiness-ledger.md` only as a derived reference to canonical requirements/matrix records; never as a second mutable status source

**Interfaces:** Consume `LanguageId`, LSP registry, DAP adapter resolution policy, terminal policy and completion-register records. Produce mapped `COMP-LANG-RUST-*`, `COMP-LANG-TS-*`, `COMP-LANG-JS-*`, and `COMP-LANG-PY-*` requirements plus fixture repository revision/lockfile hashes; acceptance remains unassessed until the required native evidence is reviewed.

- [ ] Add failing registry/matrix tests for every declared toolchain entry and an invalid unpinned server/adapter version.
- [ ] Select supported versions through an owner-reviewed contract, not `PATH` opportunism; record unavailable toolchains as blocked.
- [ ] Run `cargo test -p legion-lsp --test registry_contract`; expected: pass only when the matrix and registry agree.
- [ ] Review that each language has multi-file navigation/refactor, failing/passing test, real breakpoint/step/inspect, setup and restart/failure journey. Commit only the contract and fixtures.

### S2-02: Generalize language-server provisioning, discovery and lifecycle

**Dependencies:** S2-01.
**Owner model:** Sol engineer; Sol reviewer.
**Deliverable:** App-owned LSP discovery/provisioning/lifecycle supports the exact Rust, TS/JS and Python matrix instead of only Rust behavior. Startup is explicit and visible, failures/backoff/restart preserve buffers, and server diagnostics/completion/hover/definition/references/rename/format/code actions travel to the same desktop projections.

**Files:**
- Modify: `crates/legion-app/src/language/{mod.rs,app_lsp.rs,session.rs,download.rs,lsp_reads.rs,translate.rs}`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `crates/legion-lsp/src/lib.rs`
- Test: `crates/legion-app/tests/{language_tooling_workflow,language_restart_policy,language_stale_snapshot,language_edit_proposal_routing}.rs`
- Test: `crates/legion-lsp/tests/registry_contract.rs`

**Interfaces:** Extend existing language registry/session interfaces with approved language descriptors; preserve explicit `CommandDispatchIntent::{LspStartSession,LspRestartSession}` semantics unless the accepted S2 design changes them. Workspace edits must continue through `language::translate` and proposal authority.

- [ ] Write failing live-process integration tests for a supported TS/JS and Python server plus an unavailable-server/backoff/restart case. Use server output plus externally verified file/proposal effects, not mocked JSON replies.
- [ ] Implement registry/process configuration and path/environment discovery from the approved matrix; deny download unless its existing capability policy is granted.
- [ ] Run targeted `legion-app` language tests, `cargo test -p legion-lsp --test registry_contract`, and the standing `cargo run -p xtask -- rust-analyzer-smoke`; expected: the latter continues to run its ignored real-server protocol tests. This protocol smoke is strong integration evidence, but does not replace the native user workflow. Run against all declared servers where environment is provisioned; otherwise record blocked, do not substitute mock.
- [ ] Native scenario `language-server-lifecycle`: open each fixture, start server via shipped command, request completion/hover/definition/references, introduce/fix diagnostic, force server termination, restart, and verify retained buffer plus recovered real server state. Create the separate evidence-only commit after independent language-workflow review.

### S2-03: Deliver real navigation, formatting, code actions and refactoring through reviewable workspace edits

**Dependencies:** S2-02, S1-05.
**Owner model:** Terra; Sol review for proposal/write authority.
**Deliverable:** For every core language, user-visible completion, diagnostics, hover, definition, references, rename, formatting and code actions work against real supported server responses; multi-file edits preview, apply/cancel/rollback through existing proposal authority.

**Files:**
- Modify: `crates/legion-app/src/language/{lsp_reads.rs,proposal.rs,translate.rs,call_hierarchy.rs}`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `crates/legion-desktop/src/{bridge.rs,view.rs,workflow.rs}`
- Test: `crates/legion-app/tests/{language_edit_proposal_routing,workspace_replace_proposal}.rs`
- Test: `crates/legion-desktop/tests/language_terminal_workflow.rs`

**Interfaces:** Reuse `DesktopAction::{RequestCompletion,RequestHover,RequestRenameProposal}` and existing language proposal translation. Do not apply server `WorkspaceEdit` directly to disk or an editor buffer.

- [ ] Add one failing integration test per language that requests a multi-file rename, asserts proposal preview, changes one target externally, then asserts stale rejection/no partial disk writes; add formatting and code-action tests with externally read outputs.
- [ ] Implement missing action-to-projection paths and translation edge cases observed from real server payloads; retain URI/case normalization safeguards in `translate.rs`.
- [ ] Run language routing tests and the selected fixture-specific test commands; expected: pass for provisioned tools.
- [ ] Native scenario `language-refactor-review`: use each server to navigate, rename across files, format and apply a code action; inspect proposal diff, cancel once, apply once, run external language command, and verify changed files/diagnostics. Commit with proposal-authority reviewer approval.

### S2-04: Build and test workflow adapters for all three language groups

**Dependencies:** S2-01, S1-06.
**Owner model:** Terra.
**Deliverable:** Task/build and test discovery/run/group/targeted failure output are accurate for Rust Cargo projects, TypeScript/JavaScript browser/Node projects, and Python applications/packages in isolated environments. The test surface receives actual process result/exit state, not fabricated `VerificationRunState` rows.

**Files:**
- Modify: `crates/legion-app/src/{test_explorer.rs,lib.rs,terminal_policy.rs}`
- Modify: `crates/legion-desktop/src/{bridge.rs,view.rs}`
- Create: `crates/legion-app/tests/core_language_test_workflow.rs`
- Test: `crates/legion-app/tests/test_explorer_workflow.rs`
- Test: `crates/legion-desktop/tests/terminal_reachability.rs`

**Interfaces:** Extend the authoritative test-run descriptor/result model selected after reading `test_explorer.rs`; do not make UI rows source of truth. Reuse PTY/task policy where interactive output is needed and ensure an exit status is carried to `TestRunSummary`.

- [ ] Write a failing test for each fixture whose intentionally failing test emits language-specific output and non-zero exit, then fix source and assert a passing external result. Add a test proving a UI `Tests` click cannot mark a result passed before process completion.
- [ ] Implement adapters from the S2-01 contract and clear unavailable-runner diagnostics.
- [ ] Run each fixture command through the app route plus `cargo test -p legion-app --test core_language_test_workflow`; expected: correct discovered items, command, exit result and output provenance.
- [ ] Native scenario `language-build-test`: use the Tests/terminal surfaces to run a failing targeted and group test, inspect output, repair source, rerun passing test, and verify the host exit/output artifact for Rust, Node/browser TS/JS and Python venv projects. Create the separate evidence-only commit after independent external-oracle review.

### S2-05: Design and deliver real debug-adapter support for each supported language

**Dependencies:** S2-01, S1-06.
**Owner model:** Sol engineer and Sol reviewer.
**Deliverable:** An accepted adapter architecture maps each core language to named real adapters/launch configuration discovery, breakpoints, launch/attach as promised, step, variables, expression evaluation, debug console, terminate and crash recovery. Existing fixture/fake DAP tests remain regression-only.

**Files:**
- Create: the next unused ADR under `plans/adrs/` for core-language debug adapters; allocate the number and record the exact path in the completion-program decisions record before implementation
- Modify: `crates/legion-debug/src/{adapter_resolve.rs,live_session.rs,dap.rs,state.rs}`
- Modify: `crates/legion-app/src/{debug_workflow.rs,lib.rs}`
- Modify: `crates/legion-desktop/src/{bridge.rs,view/debug_inspector.rs,workflow.rs}`
- Test: `crates/legion-debug/tests/{system_adapter_launch_step_dogfood,live_dap_handshake,adapter_resolution_policy}.rs`
- Test: `crates/legion-app/tests/debug_workflow.rs`
- Test: `crates/legion-desktop/tests/debug_reachability.rs`

**Interfaces:** The ADR names exact DAP transport/arguments/capabilities for Rust, Node/TypeScript and Python; it specifies config discovery, policy/allowlist changes, env redaction, process cleanup and the distinction between fixture, in-tree fake, and live adapter evidence. No adapter becomes allowed merely because it is on `PATH`.

- [ ] Write the ADR and a failing resolution-policy test for each approved adapter; the test must reject a shell/unknown executable and absent adapter.
- [ ] Implement only the accepted adapter descriptors and workflow paths; preserve DAP framed transport and capability policy.
- [ ] Add live integration test for a breakpoint/step/variable result on each provisioned adapter, with fixture-mode tests explicitly labelled non-acceptance.
- [ ] Run named debug test suites; expected: real adapter proves launch, breakpoint hit, step and variable oracle, while unavailable adapter is a blocked matrix entry.
- [ ] Native scenario `language-debug`: launch each fixture from Run and Debug, set breakpoint through UI, verify a real debuggee stops, step, read variable, evaluate expression, terminate; forcibly kill adapter and prove clear error plus recoverable rerun. Create the separate evidence-only commit after security/process-lifecycle review.

### S2-06: Full native language journeys and regression gate

**Dependencies:** S2-02 through S2-05 and Stage 1 exit.
**Owner model:** Terra for harness/evidence; Astra and independent reviewer decide acceptance.
**Deliverable:** Three native, packaged, representative-repository journeys and a non-PR live-product gate that collects immutable evidence without making paid/live infrastructure compulsory for ordinary PRs.

**Files:**
- Create: `plans/acceptance/scenarios/{rust,typescript-javascript,python}-core-journey-v1.md`
- Modify: `xtask/src/main.rs`
- Modify: `.github/workflows/legion-smoke.yml` or add a specifically documented dispatched workflow after owner approval
- Modify: `docs/OPERATOR_RUNBOOK.md`
- Modify: `plans/product-readiness-ledger.md` only as a derived reference to canonical evidence; never as a second mutable acceptance status

**Interfaces:** Consume `native-product-acceptance` from S1-02 and the S2 matrix. Produce one canonical JSON `EvidenceRun` per language/OS/build with interaction capture reference, actual process outputs/exits, Git evidence, debug evidence, recovery observations and reviewer decision. TOML reports, when retained from legacy smokes, are attachments only.

- [ ] Write failing evidence-validator fixtures for an injected action trace, missing host oracle, a fake DAP-only result, missing recovery, and an unreviewed scenario.
- [ ] Implement collection/validation with no secret values in artifacts and no implicit remote/provider use.
- [ ] Run the dispatched/local workflow for every provisioned matrix entry; expected: report pass, fail or blocked, never silent skip/pass.
- [ ] Independent user scenario: open unfamiliar fixture, navigate real symbols, make multi-file change, perform real refactor, build/test/debug/fix, commit via UI, kill/restart LSP or adapter, restart IDE and confirm recovery. Verify `git log`, disk files, child exits and debug/test artifacts outside app. Review and commit evidence separately from code.

### Stage 2 exit checklist

- [ ] Each declared Rust, TypeScript, JavaScript and Python configuration has accepted live LSP, refactor, build/test and debug evidence. Any explicit blocker remains on the critical path and prevents the Stage 2 exit.
- [ ] Completion/diagnostics/hover/navigation/refactoring and task/test/debug results came from named real supported tools, not mocks, status text, fixture DAP or prebuilt verification rows.
- [ ] Every workspace mutation travelled through the app/project proposal authority and stale/conflict/cancel/restart cases preserved user work.
- [ ] Packaged native journeys on Windows/macOS/Linux have independent reviewer evidence and all earlier Manual scenarios still pass.

## Cross-stage verification and handoff

- [ ] After each package: run the targeted tests listed in that package, inspect its diff, attach external artifact evidence, run the applicable `xtask` validator, and obtain an independent review before changing acceptance to `accepted`.
- [ ] Before any Stage exit: run `cargo run -p xtask -- docs-hygiene`, `cargo run -p xtask -- claim-audit`, `cargo run -p xtask -- verify-readiness-consistency`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, targeted tests, and the relevant native scenarios. Full phase gates follow the repository gate policy once the candidate is stable.
- [ ] Keep source code commits narrow: one reviewable behavioral package per commit, then a separate evidence/register commit. Do not commit credentials, tool binaries, private logs, raw document text or screenshots containing user data.
- [ ] Escalate any new persistent-layout authority, terminal/process lifecycle, LSP/DAP transport, protocol/dependency-policy, cross-platform input, or security-policy design to Sol/Astra before implementation; record the accepted bounded design in the named ADR rather than inventing interfaces in a feature task.
