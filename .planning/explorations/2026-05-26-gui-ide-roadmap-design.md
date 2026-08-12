# Design Exploration - Devil IDE GUI Productization Roadmap

## Initial Ask

The user invoked `legion explore` and asked to create a roadmap and milestone plan from the current Devil IDE state to a fully functional IDE with a GUI.

This exploration uses the current checkout as the source of truth, not the older architectural roadmap alone. It does not run `/legion:start`; it produces a design document that can be used as start input later.

## Research Summary

- Facts:
  - The repository is a Rust 2024 workspace with crates for app composition, UI projections, editor/text, workspace/project, protocol, security, storage, observability, semantic index, AI, agents, plugins, collaboration, remote development, terminal, telemetry, retention, and CLI tooling.
  - `.planning/` did not exist before this exploration; existing planning state lives under `plans/`.
  - `crates/devil-app/src/main.rs` is still a command-line proof entrypoint. It opens the current directory as a trusted workspace, opens one file, and supports `:w` and `:q`.
  - `crates/devil-ui` is projection-only. It depends on `devil-protocol` and `thiserror`, exposes `ShellProjectionSnapshot`, `ActiveBufferProjection`, and `CommandDispatchIntent`, and currently renders through ANSI/plain terminal output in `Shell::render`.
  - There is no current GUI framework dependency in `Cargo.toml`, `Cargo.lock`, or `crates/devil-ui/Cargo.toml` for GPUI, egui/eframe, winit, wgpu, Slint, Tauri, WRY, GTK, or similar GUI stacks.
  - Current code-level verification passed for `':q' | cargo run -p devil-app -- test.rs` and `cargo check -p devil-app --all-targets`.
  - `plans/spikes/SPIKE-001A-result.md` accepts the native shell proof with renderer reservations: GPU/compositor frame variance, native IME, native clipboard, native focus, and accessibility tree validation remain renderer-integration follow-ups.
  - `plans/phase-status-ledger.md` still says Phase 8 is future-gated, but newer Phase 8 artifacts under `plans/evidence/phase-8/` say Phase 8 acceptance and release readiness were archived on 2026-05-26. This is a source-of-truth conflict that should be reconciled before the next milestone is treated as authoritative.
  - Local architecture docs recommend a Rust-native GPU/editor shell direction, but current external primary sources make the renderer choice non-trivial: GPUI is still pre-1.0 and its README currently says macOS or Linux; egui/eframe supports native Windows/macOS/Linux and web/mobile targets but is immediate-mode and not native-looking; Tauri/WRY uses OS webviews and message passing; AccessKit is the current Rust accessibility infrastructure to plan around for custom UI toolkits.
- Inferences:
  - The core substrate is much more advanced than the user-visible app. Devil IDE has protocol, proposal, policy, storage, semantic, AI/trust, plugin, collaboration, remote, terminal, and hardening work in the repo, but the shipping surface is still not a GUI IDE.
  - The next roadmap should be a productization track, not another platform-substrate track. The critical path is renderer-backed GUI integration, native input/accessibility/platform behavior, user-visible editor workflows, and packaging.
  - The first milestone must preserve the existing invariant that UI renders projections and emits intents only. A GUI adapter must not own editor buffers, workspace state, proposal lifecycle state, terminal sessions, AI provider calls, or mutation authority.
  - Because the project has Windows-first evidence and GPUI's current public README does not claim Windows support, the renderer milestone must include a decision gate rather than assuming GPUI as the dependency.
- Assumptions:
  - "Fully functional IDE with GUI" means at minimum: open workspace, browse files, open/edit multiple files, save safely, handle conflicts, search, run terminal commands under policy, surface diagnostics/completions/code actions through LSP/proposal paths, show proposal/trust surfaces, and package a desktop app.
  - The first release target remains Windows-first, with macOS/Linux parity as a release-hardening gate.
  - AI, plugin, collaboration, and remote surfaces should be visible through the GUI only after the core local IDE workflows are usable.
  - The roadmap should create buildable milestone boundaries, not a calendar forecast.

## Product Definition

- Target users:
  - Developers who want a local-first, control-first IDE with strong safety around saves, generated edits, AI assistance, terminal execution, plugins, collaboration, and remote work.
  - Initial internal users validating Devil IDE on real repositories.
  - Later external users who need a daily-driver desktop IDE.
- Primary outcome:
  - Convert the existing CLI/projection substrate into a renderer-backed desktop IDE that can be used for normal development workflows.
- Value proposition:
  - A Rust-native, control-first IDE where GUI actions are fast and visible, while every non-user-direct mutation remains proposal-mediated, auditable, reversible where supported, and privacy-aware.
- Non-goals:
  - Do not rebuild the editor, workspace, proposal, semantic, AI, plugin, collaboration, remote, or terminal substrates inside GUI code.
  - Do not add a GUI framework as a drive-by dependency without a renderer ADR, proof spike, dependency-policy update, and platform evidence.
  - Do not ship autonomous AI mutation before proposal ledger, context manifest, approval, rollback, and audit surfaces are usable in the GUI.
  - Do not claim "fully functional IDE" from CLI proof or projection tests alone.

## Recommended Approach

Use a balanced productization track: add a new renderer-backed desktop adapter around the existing projection and intent boundary, then promote one user-visible workflow at a time until the IDE is daily-drivable.

The key architectural move is to keep `devil-ui` as the projection model and command-intent layer, and add a separate renderer/application-shell integration crate or binary such as `devil-desktop` only after a renderer ADR is accepted. That GUI adapter consumes `ShellProjectionSnapshot` and emits `CommandDispatchIntent` values. `devil-app` remains the composition and authority layer for editor, workspace, proposal, AI, plugin, collaboration, remote, terminal, storage, policy, and observability.

Renderer selection should be gated by a short proof:

- Primary strategic target: Rust-native editor-grade renderer with strong control over text, GPU composition, key input, IME, clipboard, focus, accessibility, and multi-panel layout.
- Fast fallback: egui/eframe if the goal is a Windows-first usable desktop GUI quickly and the immediate-mode tradeoffs are acceptable.
- Panel fallback: Slint or Tauri/WRY only for auxiliary panels or early product validation if native-editor rendering is not ready.
- Accessibility requirement: plan an AccessKit tree or equivalent from the first renderer milestone, not after visual polish.

This approach is recommended because the local codebase already has the hard safety substrate. The highest risk now is accidentally bypassing that substrate in the GUI, or choosing a renderer that cannot satisfy editor-grade input/accessibility/platform requirements.

## Alternatives Considered

| Approach | Strengths | Tradeoffs | Decision |
|----------|-----------|-----------|----------|
| Conservative: keep CLI/TUI and increment projection coverage | Lowest risk to core architecture; no dependency decision yet | Does not satisfy the user's GUI goal; cannot validate compositor, IME, clipboard, focus, or accessibility | Rejected as the primary path |
| Balanced: renderer-backed desktop adapter over existing projections | Preserves existing boundaries; creates real GUI milestones; allows renderer proof before commitment | Requires a new ADR, dependency policy, and platform evidence before implementation | Recommended |
| Ambitious: adopt a full native GPU/editor framework immediately | Fastest route to a Zed-like product shape if the framework fits | High framework/platform risk, especially Windows-first support and pre-1.0 churn | Use only after the renderer proof passes |
| Hybrid: Tauri/WRY GUI over Rust backend | Fast layout/tooling, accessible web controls, familiar frontend ecosystem | WebView/editor performance and cross-webview inconsistency risks; creates IPC surface to secure | Keep as fallback or auxiliary-panel option |

## Feature Scope

### MVP

- [ ] Reconcile current planning truth: update the phase ledger or create a GUI-specific baseline that resolves the Phase 8 ledger/evidence conflict.
- [ ] Accept a renderer integration ADR that chooses the GUI stack and records fallback criteria.
- [ ] Add a renderer-backed desktop adapter that opens a native window and renders layout, explorer, active buffer viewport, status, and proposal/trust summaries from projections.
- [ ] Route window/menu/key/command events into existing `CommandDispatchIntent` and `AppComposition` handling.
- [ ] Support workspace open, file tree browsing, multi-tab open/edit/close, cursor/selection, insert/delete/replace, undo/redo, and save.
- [ ] Surface save outcomes, stale/conflict/denied proposal responses, dirty indicators, and rollback/checkpoint availability.
- [ ] Add command palette, find-in-file, find-in-workspace, and basic settings/keybinding projection.
- [ ] Add LSP diagnostics, hover, completion, formatting, rename, and code-action GUI surfaces where edit-producing actions become proposals.
- [ ] Add a policy-gated terminal panel with bounded output and clear denial/error states.
- [ ] Add proposal ledger, preview, approve/reject/apply/rollback/cancel, context manifest, privacy inspector, and permission budget panels.
- [ ] Add renderer-backed performance evidence: p50/p95 input-to-paint, frame-time variance, large-file degraded mode, and non-blocking background work.
- [ ] Add native clipboard, IME, focus traversal, keyboard shortcut, menu, file dialog, accessibility tree, and theme evidence.
- [ ] Package a Windows desktop app with a documented smoke-test path.

### Later

- [ ] macOS and Linux parity with the same renderer/input/accessibility evidence.
- [ ] Rich semantic navigation: outline, go-to-definition, references, symbol search, test impact, and semantic context previews.
- [ ] Assisted AI GUI flows with local-first provider routing, visible context manifests, proposal-only edits, and no hidden egress.
- [ ] Delegated task command center with task plans, checkpoints, proposed diffs, verification output, and no self-approval.
- [ ] Plugin contribution UI, sandbox status, plugin settings, host-call diagnostics, and marketplace governance only after plugin gates are accepted.
- [ ] Collaboration presence, shared proposals, reconnect state, and conflict review UI.
- [ ] Remote workspace connection manager, latency/reconnect indicators, remote terminal/LSP status, and offline-resume review.
- [ ] Release-quality update, crash reporting, diagnostics export, enterprise policy profiles, and controlled telemetry.

## Experience / Workflow

1. User launches the desktop app with an optional workspace path.
2. The app opens a native window and projects the current workspace: explorer on the left, editor tabs and viewport in the center, status/trust indicators around the edges, terminal/proposal/problems panels below.
3. User opens a file from the explorer. The GUI renders a viewport slice, not full source by default.
4. User edits text. GUI input emits command intents, `AppComposition` routes to `EditorEngine`, and the GUI receives a new projection snapshot.
5. User saves. The existing proposal-mediated save path validates preconditions and writes through workspace authority. Rejected saves preserve dirty text and show a conflict/denial panel.
6. User invokes completion, code action, format, rename, AI assist, plugin command, terminal command, collaboration action, or remote operation. Every mutation-capable output is represented as a proposal preview before apply.
7. User reviews proposals in a ledger, sees affected targets, privacy/risk labels, context provenance, and rollback availability, then approves, rejects, applies, cancels, or rolls back.
8. The app records metadata-only events, storage records, and diagnostics without persisting raw source, prompts, provider payloads, secrets, or unbounded terminal output by default.

## Milestone Roadmap

### M0: Baseline Reconciliation and Renderer Decision

**Goal**: Establish the exact current state and choose a GUI renderer path without weakening architecture gates.

**Must deliver**:
- Update or supersede the phase ledger so it agrees with the newer Phase 8 evidence, or explicitly mark the conflict as unresolved for this GUI track.
- Add a renderer integration ADR that evaluates Rust-native GPU, egui/eframe, Slint, and Tauri/WRY against Windows-first GUI IDE requirements.
- Define `devil-desktop` or equivalent crate/binary boundaries.
- Update dependency policy and `xtask` checks before introducing renderer dependencies.
- Preserve `devil-ui` as projection-only and GUI-adapter-owned rendering only.

**Definition of done**:
- `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, and targeted app/UI tests pass.
- Renderer choice has explicit fallback criteria for input latency, text rendering, IME, clipboard, focus, accessibility, platform menus, and packaging.
- No GUI dependency is added without policy coverage.

### M1: Renderer-Backed Foundation Mode

**Goal**: Open a real desktop window that renders the existing shell projections and routes commands through existing app authority.

**Must deliver**:
- Native window/event loop.
- Projection-to-view adapter for layout, explorer, active buffer viewport, status, proposal summary, and trust summary.
- Input/key/menu command mapping to `CommandDispatchIntent`.
- File open and save from GUI.
- Basic external conflict/denial/error display.
- Renderer-backed p50/p95 input-to-paint harness.

**Definition of done**:
- A user can launch the GUI, open this repository, open a file, edit a small buffer, save it, quit, and see rejection/conflict outcomes.
- UI code has no dependency on editor/project/storage internals beyond protocol/projection contracts approved by policy.
- Renderer proof records Windows input-to-paint, frame variance, clipboard, focus, and accessibility smoke results.

### M2: Daily Editing MVP

**Goal**: Make local editing usable enough for real files and repeated sessions.

**Must deliver**:
- Multi-tab editor model and tab UI.
- Explorer expand/collapse/selection/reveal.
- Cursor, selection, scrolling, viewport, small-buffer preview, large-file degraded mode.
- Undo/redo, save all, close dirty file prompts, reload/keep-both/conflict handling.
- Session restore for open workspace, tabs, focus, layout, and explorer state.
- Search in file and search in workspace.

**Definition of done**:
- Daily edit/save/search workflows work in the GUI without CLI fallback.
- External overwrite between open and save yields visible conflict and preserves dirty text.
- Large files never require full-source GUI projection outside bounded small-buffer mode.

### M3: Language and Terminal IDE Loop

**Goal**: Add the minimum language-tooling and terminal workflow expected from an IDE.

**Must deliver**:
- Problems panel, diagnostics underlines/list, hover, completion, go-to-definition/references, outline.
- Formatting, rename, organize imports, and code actions represented as proposal previews before mutation.
- Policy-gated terminal panel with launch/input/resize/kill/output projection, bounded transcript, and denial states.
- Command palette commands for language and terminal actions.

**Definition of done**:
- A Rust workspace can be opened, edited, checked through terminal or LSP workflows, and navigated through GUI surfaces.
- LSP and terminal cannot mutate buffers or disk directly.
- Terminal and LSP failures are visible, cancellable where applicable, and metadata-audited.

### M4: Control, Trust, and Assisted AI Surfaces

**Goal**: Make the control-first differentiator visible and usable.

**Must deliver**:
- Proposal ledger, proposal details, diff/target summary, approval checklist, rollback/checkpoint panel.
- Context manifest and privacy inspector panels.
- Permission/risk/cost budget UI.
- Assisted AI explain/propose flows using local-first/default-deny provider routing.
- Delegated task plan projection without autonomous apply.

**Definition of done**:
- AI-generated edits are proposals only.
- Users can see what context was used, why, and what was redacted or denied.
- Approval, rejection, cancellation, stale, conflict, failed, applied, and rolled-back states are visible in the GUI.

### M5: Packaging, Platform Integration, and Accessibility

**Goal**: Turn the GUI into an installable desktop application with credible platform behavior.

**Must deliver**:
- Windows installer or packaged executable.
- Native menus, file dialogs, clipboard, keyboard shortcuts, theme, focus traversal, IME, high-DPI behavior, and accessibility tree.
- Crash-safe session restore and diagnostics export.
- Smoke-test scripts for install, launch, open workspace, edit/save, terminal, LSP, proposal review, and quit.
- macOS/Linux parity plan and initial CI smoke coverage.

**Definition of done**:
- A non-developer can install and run the Windows GUI build.
- Accessibility, IME, clipboard, focus, and packaging evidence is archived.
- Global phase gates and GUI smoke tests pass.

### M6: Fully Functional Local IDE Beta

**Goal**: Reach a beta that can be used as a local IDE for normal development.

**Must deliver**:
- Stable local project workflow: open, browse, edit, search, save, run terminal, use language features, inspect proposals.
- GUI-visible diagnostics and operational health.
- Privacy-safe logs, redacted diagnostics export, and release-readiness checklist.
- Documentation for launch, common workflows, and known limitations.

**Definition of done**:
- Devil IDE can be daily-driven on at least one real Rust repo for local development without returning to the CLI shell.
- Critical workflows have automated smoke coverage and manual evidence.
- Known limitations are documented without claiming unsupported platform or AI/autonomy behavior.

### M7: Advanced Platform GUI GA

**Goal**: Expose the accepted advanced runtime surfaces through production-grade GUI workflows.

**Must deliver**:
- Plugin management and contribution views.
- Collaboration presence, shared proposal review, reconnect/conflict surfaces.
- Remote workspace connection manager and remote terminal/LSP/session status.
- Delegated task command center and bounded autonomy only after separate approval gates.
- Cross-platform release, update, rollback, and incident response procedures.

**Definition of done**:
- Advanced surfaces are usable from the GUI without bypassing proposal, policy, event, storage, privacy, or projection boundaries.
- Platform parity evidence exists for Windows, macOS, and Linux.

## First Milestone Detail - M1 Renderer-Backed Foundation Mode

### Proposed work packages

- M1.1 Renderer adapter scaffold:
  - Create the renderer crate/binary after M0 ADR/policy acceptance.
  - Wire app startup, window creation, event loop, and `AppComposition`.
  - Keep all editor/workspace/proposal authority out of GUI widget state.
- M1.2 Projection renderer:
  - Render `ShellLayoutProjection`, `ExplorerProjection`, `ActiveBufferProjection`, status messages, and summary trust/proposal projections.
  - Render viewport slices and bounded small-buffer preview only.
- M1.3 Intent bridge:
  - Map keyboard/menu/window/file-dialog actions to `CommandDispatchIntent`.
  - Route intents through app-owned handlers.
  - Add tests proving GUI intent mapping does not mutate editor/workspace directly.
- M1.4 Open/edit/save loop:
  - Launch with workspace path.
  - Open file from explorer or dialog.
  - Insert/delete/replace in active buffer.
  - Save through existing proposal-mediated path.
  - Show saved/rejected/conflict outcomes.
- M1.5 Renderer evidence harness:
  - Measure p50 and p95 input-to-paint, frame variance, focus behavior, clipboard smoke, IME smoke, and accessibility tree smoke on Windows.
  - Record fallback criteria outcomes.
- M1.6 Documentation and smoke path:
  - Add `cargo run -p <desktop-crate> -- <path>` or equivalent launch path.
  - Add a scripted smoke check if the renderer supports automation.
  - Record manual run steps and limitations.

### M1 stop gates

- Stop if the GUI needs direct access to `EditorEngine`, `WorkspaceActor`, storage repositories, proposal lifecycle state, terminal sessions, provider calls, plugin hosts, remote sessions, or collaboration runtimes.
- Stop if large-file rendering requires unbounded full-source projection.
- Stop if renderer latency cannot be measured.
- Stop if accessibility or IME has no credible implementation path.
- Stop if dependency policy cannot express the renderer stack cleanly.

## Technical Direction

### Architecture

- Keep `devil-ui` as projection and intent model.
- Add a renderer adapter crate only after M0. Candidate name: `devil-desktop`.
- `devil-desktop` depends on `devil-app`, `devil-ui`, `devil-protocol`, and renderer-specific crates approved by policy.
- Avoid renderer dependencies in `devil-editor`, `devil-project`, `devil-text`, `devil-index`, `devil-ai`, `devil-plugin`, `devil-collaboration`, `devil-remote`, `devil-terminal`, `devil-storage`, `devil-security`, and `devil-observability`.
- Prefer an adapter boundary:
  - Input: `ShellProjectionSnapshot` and future GUI-specific projection DTOs.
  - Output: `CommandDispatchIntent` and explicit app-level requests.
  - Side effects: none except renderer/window/platform UI side effects.

### Renderer decision criteria

- Windows-first viability.
- Text rendering control for code editor features.
- Stable key input, IME, clipboard, focus, high-DPI, menus, file dialogs, and drag/drop.
- Accessibility tree support, preferably via AccessKit or a platform-equivalent path.
- p50/p95 input-to-paint measurement support.
- Cross-platform path to macOS/Linux.
- Dependency policy fit and supply-chain risk.
- Ability to keep GUI state projection-only.

### Testing and verification

- Existing gates:
  - `cargo run -p xtask -- check-deps`
  - `cargo fmt --all --check`
  - `cargo check --workspace --all-targets`
  - `cargo test --workspace --all-targets`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo deny check`
- GUI-specific gates:
  - Renderer smoke launch.
  - Headless or automation-backed projection render test if available.
  - Input-to-paint p50/p95 harness.
  - Frame variance harness.
  - Clipboard, IME, focus, accessibility, high-DPI, and file-dialog smoke tests.
  - Save conflict regression from GUI command path.
  - Large-file viewport/degraded-mode regression from GUI command path.

### External research references

- GPUI README, current public state: https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md
- egui/eframe README, current public state: https://github.com/emilk/egui
- AccessKit overview: https://accesskit.dev/
- Tauri architecture and WRY/TAO model: https://v2.tauri.app/concept/architecture/
- Slint documentation overview: https://docs.slint.dev/

## Open Questions

- Which renderer stack should be approved for M1? Resolution path: M0 renderer ADR and spike.
- Should the Phase 8 ledger/evidence conflict be fixed in `plans/phase-status-ledger.md`, or should the GUI track create its own baseline without editing historical phase status? Resolution path: M0 baseline reconciliation.
- Is "fully functional IDE" scoped to local daily-driver workflows first, or should remote/collaboration/plugin/AI delegation be part of the first beta? Resolution path: pick M6 local beta as default; defer advanced GUI GA to M7 unless user explicitly expands beta scope.
- Should the first GUI crate be named `devil-desktop`, `devil-gui`, or become a new binary under `devil-app`? Resolution path: M0 crate boundary decision.
- What is the first supported platform after Windows? Resolution path: M5 platform parity plan.

## Start Input

Initialize a Legion project/roadmap for GUI productization of the existing Devil IDE repository. Current state: Rust workspace with advanced app/protocol/proposal/semantic/AI/plugin/collaboration/remote/terminal substrate, but only a CLI proof entrypoint and projection-only ANSI/plain UI rendering. Goal: build a fully functional desktop IDE GUI without violating the existing architecture invariants: UI consumes projections and emits intents only, editor owns text transactions, workspace owns file authority, all non-user-direct mutations are proposal-mediated, observability/storage are metadata-only by default, and runtime surfaces remain policy-gated.

Recommended initial roadmap:

- M0: Baseline reconciliation and renderer decision.
- M1: Renderer-backed foundation mode.
- M2: Daily editing MVP.
- M3: Language and terminal IDE loop.
- M4: Control/trust and assisted AI surfaces.
- M5: Packaging, platform integration, and accessibility.
- M6: Fully functional local IDE beta.
- M7: Advanced platform GUI GA.

First build milestone should be M1 only after M0 is accepted. M1 delivers a native window, projection renderer, intent bridge, open/edit/save loop, conflict/rejection UI, and renderer evidence harness while preserving projection-only UI boundaries.
