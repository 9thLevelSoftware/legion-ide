# Four-Mode UI Prototype Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` to execute this plan task by task with a fresh implementer and task-scoped reviewer for every task.

**Goal:** Turn the accepted Legion IDE functional prototype into the native app's polished, functional four-mode shell while preserving Legion's existing ownership, safety, and proposal-mediated execution contracts.

**Architecture:** Retheme and recompose the existing `egui` desktop renderer around the real `ShellProjectionSnapshot` and `DesktopAction` seams. The renderer may own only ephemeral presentation state such as a pending confirmation or task-input draft. Product/editor/workspace/workflow state remains app-owned and every operation continues through `DesktopAction -> DesktopCommandBridge -> CommandDispatchIntent/DesktopAppRequest -> DesktopRuntime/AppComposition`.

**Tech Stack:** Rust 2024, `egui`/`eframe`, existing `legion-ui` projections, `legion-desktop` renderer/bridge/workflow, Cargo tests, PowerShell + Microsoft Edge only for local reference capture.

## Accepted reference

- Source artifact: `C:\Users\dasbl\Downloads\Legion IDE prototype.zip`
- Extracted local reference: `D:\tmp\legion-prototype-reference-20260806`
- Accepted 1440x900 captures: `manual-1440x900.png`, `assist-1440x900.png`, `delegate-1440x900.png`, `autonomous-1440x900.png`
- Compact diagnostic capture: `assist-960x720.png`
- ImageGen compact-layout study (reference only, never a shipped asset): `C:\Users\dasbl\.codex\generated_images\019fd524-3dce-7311-8950-d40fcccabbc4\exec-dd1f6bef-07f6-4051-b640-b45ea005aff0.png`

The artifact is authoritative for composition, density, and visual language. Repository policy is authoritative for naming and behavior where the artifact conflicts with product safety.

## Global Constraints

- Expose exactly four visible product modes in this order and with this copy: `Manual`, `Assist`, `Delegate`, `Legion Workflows`.
- Do not render `Automate`, `Autonomous`, or `Delegates` as a mode label. `DockMode::Automate` remains a compatibility/internal binding to the visible `Legion Workflows` surface.
- Preserve the current four internal dock modes and existing action/bridge/runtime ownership path; this plan does not add a fifth mode or replace protocol-owned state.
- `legion-ui` remains projection-only. The renderer must not own editor text, buffer/session state, delegated-task runtime state, permissions, proposals, workflow state, or apply authority.
- Ephemeral renderer state is limited to interaction presentation: compact-panel visibility, pending mode confirmation, and unsent task-input drafts.
- Mode selection alone must never start a provider, worker, terminal, network route, workflow, file mutation, or permission grant.
- Manual mode must not display live agent, remote presence, CRDT-live, provider, or network-egress claims. Its first-screen right rail is an explicit AI-disengaged/zero-egress state.
- Assist continues to route only existing inline-prediction request/accept/dismiss/cancel actions.
- Delegate must launch the real scoped delegated-task path (`StartDelegatedTask`), not the proposal-only `StartAiProposal` path.
- Legion Workflows reuses existing workflow/fleet projections and retains proposal-mediated apply, risk gates, bounded permissions, approval controls, and kill switch. Do not copy the artifact's blanket “low-risk actions auto-approve” language.
- Saves and applies remain proposal-mediated and fingerprint/version/generation/correlation guarded as documented in repository `AGENTS.md`.
- Preserve dark, light, and system theme support. The accepted prototype defines the dark palette; light mode must remain usable and high-contrast rather than being removed.
- At 1440x900, match the artifact's stable shell proportions: approximately 42px top bar, 46px activity rail plus 248px explorer, 325px right inspector, 192px bottom console, and 24px status bar.
- At 960x720, use deterministic compact widths/collapse rules so the editor remains at least 360px wide and all hidden secondary panes remain reachable. No overlap, clipping, or absolutely centered top-bar collision is acceptable.
- Essential text must not be below 11px. Normal text targets 4.5:1 contrast; visible focus/UI boundaries target 3:1. All primary interactive targets must be at least 24x24px.
- Keyboard users must be able to traverse the four-mode switch, cancel an escalation with Escape, confirm it without a pointer, open the command palette, and activate visible rail actions without a focus trap.
- No permanent placeholders, fake metrics, invented presence, hard-coded successful runtime states, or `TODO`/`FIXME` markers.
- Do not commit the downloaded prototype, generated image, temporary screenshots, credentials, or private data.

---

## Task 1: Canonical four-mode contract and display normalization

**Files:**

- Modify: `crates/legion-ui/src/ui.rs`
- Modify: `crates/legion-ui/tests/assist_inline_prediction.rs` only if construction helpers require updates
- Modify: `crates/legion-desktop/src/view.rs`
- Modify: `crates/legion-desktop/tests/projection_rendering.rs`
- Modify: focused existing tests that assert obsolete `Automate`/`Delegates` display copy

### Checklist

- [ ] Add failing tests proving `DockMode::{Manual, Assist, Delegate, Automate}` renders the canonical visible labels `Manual`, `Assist`, `Delegate`, `Legion Workflows`.
- [ ] Preserve legacy parsing aliases for `Automate`, `Autonomous`, `LegionWorkflows`, and `Legion Workflows`, but normalize all output to `Legion Workflows`.
- [ ] Make `DockMode::Automate::to_product_mode()` map to canonical `ProductMode::LegionWorkflows`; do not remove legacy protocol variants needed for deserialization.
- [ ] Rename the renderer-local `DesktopProductMode::Delegates` variant to singular `Delegate` and remove obsolete user-facing `Automate`, `Autonomous`, and `Delegates` copy from mode chrome, onboarding, status, confirmation, and palette rows.
- [ ] Source switch labels/shortcuts from one four-entry renderer helper that is mechanically consistent with `CANONICAL_PRODUCT_MODES`.
- [ ] Keep the internal `DockMode::Automate` binding for layout/projection compatibility.
- [ ] Run and record RED then GREEN evidence.

### Focused verification

```powershell
cargo test -p legion-protocol --test mode_taxonomy
cargo test -p legion-ui ui::tests::dock_layouts_are_mode_scoped_and_manual_layout_is_ai_free
cargo test -p legion-ui ui::tests::dock_mode_labels_are_canonical
cargo test -p legion-desktop projection_rendering -- --nocapture
rg -n '"(Automate|Autonomous|Delegates)"' crates/legion-ui/src crates/legion-desktop/src
```

The final `rg` may find compatibility parser input or internal comments/identifiers, but it must find no user-facing mode label.

---

## Task 2: Prototype design tokens and responsive shell geometry

**Files:**

- Modify: `crates/legion-desktop/src/theme.rs`
- Modify: `crates/legion-desktop/src/view.rs`
- Modify: `crates/legion-desktop/tests/projection_rendering.rs`
- Modify: `crates/legion-desktop/tests/accessibility.rs` only for geometry-derived accessibility assertions

### Checklist

- [ ] Add failing token and geometry tests before implementation.
- [ ] Retokenize dark mode to the accepted blue-slate/warm-amber system: shell `#16202b`, editor `#121a23`, panel `#1d2a38`, borders centered on `#2c3a4a`, primary text `#f4f1eb`, muted text `#7e8a9b`, primary amber `#cf8136`, Assist blue `#2e7fb8`, success `#4fae6d`, and danger red near `#d23b2e`.
- [ ] Retain a legible light theme with the same semantic token roles; update focus tokens so they remain visible in both themes.
- [ ] Introduce a small, pure `ShellGeometry`/layout-policy helper in `view.rs` that derives panel sizes and compact behavior from available width/height. Do not scatter viewport thresholds across render functions.
- [ ] Use the reference desktop geometry at 1440x900 and a compact geometry at 960x720 that preserves a >=360px editor canvas.
- [ ] Keep left/right/bottom panels resizable at normal widths while ensuring deterministic compact defaults and minimums.
- [ ] Add an activity-rail composition inside the left region without inventing navigation state; rail buttons may expose existing dock/palette actions only.
- [ ] Replace mode-dependent outer panel widths/heights with stable shell geometry so changing modes does not shift the editor.
- [ ] Keep top and status bars fixed at reference density; avoid text smaller than 11px for essential information.
- [ ] Run and record RED then GREEN evidence.

### Focused verification

```powershell
cargo test -p legion-desktop theme::tests -- --nocapture
cargo test -p legion-desktop view::tests::shell_geometry -- --nocapture
cargo test -p legion-desktop --test projection_rendering -- --nocapture
cargo fmt --all --check
```

---

## Task 3: Prototype chrome, explorer, editor, terminal, and status composition

**Files:**

- Modify: `crates/legion-desktop/src/view.rs`
- Modify: `crates/legion-desktop/src/theme.rs` only for reusable control/frame helpers
- Modify: `crates/legion-desktop/src/code_canvas.rs` only for prototype-specific editor paint tokens; do not replace the canvas
- Modify: `crates/legion-desktop/tests/projection_rendering.rs`
- Modify: `crates/legion-desktop/tests/keyboard_nav.rs`
- Modify: `crates/legion-desktop/tests/headless_input.rs`

### Checklist

- [ ] Add failing headless/control tests for the new chrome before implementation.
- [ ] Recompose the top bar into three collision-safe regions: Legion wordmark/workspace, centered four-mode switch, and one command-palette/presence region. Remove decorative window-control dots and the row of competing command buttons.
- [ ] Route the `Command`/shortcut control to the existing command palette. Do not invent search or presence results.
- [ ] Simplify the first-screen left sidebar to the narrow activity rail plus `EXPLORER · {workspace}` and the real projected tree. Keep richer Git/debug/test surfaces reachable through existing docks/palette instead of crowding the explorer.
- [ ] Retune the real tab strip and breadcrumb bar to the reference: compact height, amber selected-tab rule, restrained close/dirty indicators, and slate separators.
- [ ] Retune `EguiCodeCanvasPainter` colors for editor background, current line, line numbers, selection, cursor, and syntax while preserving the existing editor action path.
- [ ] Make the bottom console terminal-first with `TERMINAL`, `PROBLEMS`, and `AGENT LOG` tabs projected from existing state; never render a live agent log in Manual.
- [ ] Make the status bar a single compact line with only truthful projected mode/trust/LSP/file/cursor information.
- [ ] Preserve focus, selection, scroll, user panel sizing, and settings across mode changes.
- [ ] Run and record RED then GREEN evidence.

### Focused verification

```powershell
cargo test -p legion-desktop --test projection_rendering -- --nocapture
cargo test -p legion-desktop --test keyboard_nav -- --nocapture
cargo test -p legion-desktop --test headless_input -- --nocapture
cargo test -p legion-desktop code_canvas -- --nocapture
cargo fmt --all --check
```

---

## Task 4: Functional four-mode right rails

**Files:**

- Modify: `crates/legion-desktop/src/view.rs`
- Modify: `crates/legion-desktop/src/bridge.rs` only if a missing existing action mapping is proven
- Modify: `crates/legion-desktop/tests/control_trust_view.rs`
- Modify: `crates/legion-desktop/tests/assist_inline_prediction_workflow.rs`
- Modify: `crates/legion-desktop/tests/delegated_task_command_center.rs`
- Modify: `crates/legion-desktop/tests/legion_workflow_command_center.rs`
- Modify: `crates/legion-desktop/tests/projection_rendering.rs`

### Checklist

- [ ] Add failing action-routing and projection tests for each mode before implementation.
- [ ] Manual rail: show `AI engine disengaged`, zero-egress/local-only policy copy, and an `Enable Assist` action that emits only `SetProductMode { Assist }`. Hide agent, remote presence, collaboration-live, and provider surfaces.
- [ ] Assist rail: reuse the real `AssistInlinePredictionProjection`; active suggestions expose Accept/Dismiss, idle state exposes Predict, and the list shows truthful next-edit predictions. Preserve request/accept/dismiss/cancel action mappings.
- [ ] Delegate rail: provide an adapter-local unsent task draft, scope/budget/sandbox explanation from existing projections, and a `Delegate task` CTA that emits `StartDelegatedTask { task_description, desktop_default_delegated_scope(snapshot) }`. It must not emit `StartAiProposal`.
- [ ] When Delegate is active, compactly render real phase, task/DAG, proposal, review, permission, cancel, and evidence state rather than prototype fake progress.
- [ ] Legion Workflows rail: compact existing workflow/fleet task cards, resource budgets, risk gate, approval, permission, cancel, and kill-switch controls into the prototype's stacked layout while retaining canonical copy and proposal-mediated behavior.
- [ ] No rail may synthesize a permission grant, successful task, presence avatar, budget, or approval state absent from the snapshot.
- [ ] Keep the existing richer workbench surfaces reachable; first-screen simplification must not delete capabilities.
- [ ] Run and record RED then GREEN evidence.

### Focused verification

```powershell
cargo test -p legion-desktop --test control_trust_view -- --nocapture
cargo test -p legion-desktop --test assist_inline_prediction_workflow -- --nocapture
cargo test -p legion-desktop --test delegated_task_command_center -- --nocapture
cargo test -p legion-desktop --test legion_workflow_command_center -- --nocapture
cargo test -p legion-desktop --test projection_rendering -- --nocapture
cargo fmt --all --check
```

---

## Task 5: Mode-escalation confirmation and accessible interaction

**Files:**

- Modify: `crates/legion-desktop/src/view.rs`
- Modify: `crates/legion-desktop/src/platform.rs` only when existing semantic metadata needs a mode-dialog role/state
- Modify: `crates/legion-desktop/tests/headless_input.rs`
- Modify: `crates/legion-desktop/tests/keyboard_nav.rs`
- Modify: `crates/legion-desktop/tests/accessibility.rs`
- Modify: `crates/legion-desktop/tests/projection_rendering.rs`

### Checklist

- [ ] Add failing transition-policy, cancel, confirm, and keyboard tests before implementation.
- [ ] Add renderer-owned `pending_mode_confirmation: Option<DockMode>` presentation state; it is not product state and grants no authority.
- [ ] Encode confirmation policy with explicit named matches, never ordinal arithmetic: entering Delegate from Manual/Assist requires confirmation; entering Legion Workflows from Manual/Assist/Delegate requires confirmation; Manual/Assist entry and every privilege-reducing transition apply immediately.
- [ ] Clicking an escalation target leaves the projected active mode unchanged and opens a modal that explains proposal-mediated execution and bounded permissions without showing grant checkboxes.
- [ ] Confirm emits the existing `DesktopAction::SetProductMode`; Cancel and Escape emit nothing and restore focus to the switch. A subsequent snapshot remains the sole source of active-mode truth.
- [ ] Route every rendered desktop mode entry point through the same request helper so onboarding/rail/switch behavior cannot drift.
- [ ] Add semantic labels, selected/current state, dialog title/body, confirm/cancel actions, visible focus, and a non-trapping keyboard order to the existing accessibility metadata seam.
- [ ] Verify compact mode switch labels remain readable at 960x720 and at 200% zoom.
- [ ] Clearly document in code that presentation confirmation is not the execution security boundary; operation-level app gates remain authoritative.
- [ ] Run and record RED then GREEN evidence.

### Focused verification

```powershell
cargo test -p legion-desktop --test headless_input -- --nocapture
cargo test -p legion-desktop --test keyboard_nav -- --nocapture
cargo test -p legion-desktop --test accessibility -- --nocapture
cargo test -p legion-desktop --test projection_rendering -- --nocapture
cargo fmt --all --check
```

---

## Task 6: Visual evidence, fidelity ledger, and scoped release verification

**Files:**

- Create: `docs/ui/four-mode-prototype-fidelity.md`
- Modify: `crates/legion-desktop/tests/projection_rendering.rs` only for final deterministic viewport assertions
- Modify: `crates/legion-desktop/tests/accessibility.rs` only for final acceptance gaps
- Do not commit temporary screenshot outputs

### Checklist

- [ ] Launch the native app against a deterministic local workspace and capture Manual, Assist, Delegate, and Legion Workflows at 1440x900 plus Assist at 960x720.
- [ ] In one visual QA pass, inspect the accepted concept and latest implementation captures with `view_image`.
- [ ] Write a fidelity ledger with at least: shell geometry, four-mode switch, dark token palette, explorer/editor composition, mode-specific right rail, terminal/status composition, compact viewport behavior, copy differences, and focus/accessibility evidence.
- [ ] Record the intentional deviations: `Legion Workflows` replaces artifact `Autonomous`; Manual hides agent/presence surfaces; workflow mutations remain proposal-mediated; no blanket low-risk auto-approval; system fonts substitute for unbundled prototype web fonts if licensing/assets are absent.
- [ ] Record viewport method and exact capture commands, including why local Edge/reference capture was used (no Browser/IAB connector was available).
- [ ] Resolve all P0/P1 fidelity gaps discovered in the comparison before final verification. Cosmetic P2 gaps may remain only when explicitly documented with rationale.
- [ ] Run the focused package checks and the repository UI safety gates. Do not claim the broad desktop test bundle green unless a fresh full run completes.
- [ ] Run and record final verification from the exact final HEAD.

### Final verification

```powershell
cargo fmt --all --check
cargo check -p legion-ui -p legion-desktop
cargo test -p legion-ui
cargo test -p legion-desktop --tests
cargo clippy -p legion-ui -p legion-desktop --all-targets -- -D warnings
cargo run -p xtask -- check-deps
cargo run -p xtask -- claim-audit
cargo run -p xtask -- no-egui-textedit
git status --short
```

If package/full-suite timing prevents completion, report the exact command, elapsed limit, and last observed state; never convert a timeout into a passing claim.
