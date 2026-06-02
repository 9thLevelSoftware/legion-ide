# 01 — Legion IDE Front-End App Architecture Plan

Generated: 2026-06-01 16:24:53 EDT

## 0. Executive summary

Legion’s front end should be rebuilt around one central idea:

A deterministic IDE shell with mode-filtered panels, where AI capabilities are structurally unavailable in Manual mode and progressively introduced through Assist, Delegate, and Automate modes.

The immediate front-end keystone is the panel-host dock refactor described in the Discord artifact. This is not cosmetic. It is the enforcement mechanism for product modes, offline safety, and the eventual Legion fleet console.

The front end must support four product states:

1. Manual
   - No AI.
   - No inference-backed panels.
   - No remote/cloud worker controls.
   - Deterministic IDE features only.
   - Offline build must compile without `devil-ai`, `devil-ai-providers`, or `devil-agent`.

2. Assist
   - Inline, local or remote inference.
   - No chat necessary.
   - No autonomous tool execution.
   - Ghost text, inline edits, scoped refactors, AI quick actions.

3. Delegate
   - Chat with codebase context.
   - Multi-file proposal generation.
   - Human review and per-hunk approval.
   - Tool permissions with confirm/allow/deny.

4. Automate
   - Legion assembly-line/fleet workflow.
   - Kanban task graph.
   - Ephemeral specialist workers.
   - Decision feed.
   - Risk monitor.
   - Kill switch.
   - Proposal-only outputs.

The front end should make the product’s trust model visible:

- Every worker has a role.
- Every task has a bounded scope.
- Every mutation is a proposal.
- Every accepted change has evidence.
- Every risky action is gated.
- Every agent can be killed.

## 1. Current known repo state

From repo inspection and the planning artifact:

- `devil-ui` owns UI components and should own the dock/panel registry.
- `devil-desktop` should remain projection-only.
- `devil-app` owns application authority, proposals, and approval gates.
- `devil-protocol` owns DTOs and protocol-facing data.
- `devil-agent` owns delegated task runtime and Legion orchestration.
- Existing UI currently has a `RightConsole` style whole-pane swap by mode.
- The first front-end pivot is replacing that whole-pane swap with a real dock host.

Rename/pivot note:

- Rename user-facing product to Legion IDE.
- Avoid immediate internal crate rename if it slows delivery.
- First rename visible app strings, docs, branding, plan names, window titles, and user-facing commands.
- Later rename crates/packages from `devil-*` to `legion-*` in a controlled migration.

## 2. Front-end architecture principles

### 2.1 Projection-only UI

The UI must not directly mutate workspace state.

The UI may:

- Render panels.
- Dispatch user intents.
- Display diagnostics, results, proposals, evidence, and agent status.
- Send approval/rejection decisions to `devil-app`.
- Request commands through typed APIs.

The UI must not:

- Apply patches directly.
- Run arbitrary commands directly.
- Instantiate AI providers directly.
- Bypass proposal gates.
- Hide AI access behind Manual mode.

### 2.2 Shared registry, filtered by mode

There should be exactly one panel registry.

Manual mode should not have a separate hand-written panel list. Instead, it should filter the shared registry by capability metadata.

This prevents divergence and makes tests simple:

- If `requires_ai == true`, Manual cannot construct it.
- If `requires_network == true`, Offline build cannot construct it.
- If `requires_cloud == true`, local-only builds cannot construct it.

### 2.3 Mode-specific layouts

Each mode needs its own persisted layout.

Manual layout:

- Left: Project, Outline, Git.
- Right: Inspector, Structural Search, Dependencies/Security.
- Bottom: Terminal, Problems, Search Results, Test Results.

Assist layout:

- Manual panels plus inline prediction settings.
- AI inline suggestions visible in editor.
- No chat/fleet console by default.

Delegate layout:

- Right: Chat, Approval Queue, Context Inspector.
- Bottom: Diff Review, Agent Logs, Terminal.

Automate layout:

- Left or right: Legion Board / Fleet Console.
- Right: Worker Details, Risk Monitor, Decision Feed.
- Bottom: Evidence, Logs, Validation Results, Integration Diff.

### 2.4 Deterministic-first UX

The deterministic IDE should feel valuable before the AI layer lands.

Manual mode must include:

- Outline.
- Breadcrumbs.
- Sticky scope headers.
- Diagnostics.
- Quick fixes.
- Find references.
- Go to definition.
- Inlay hints.
- Code lens.
- Structural Search & Replace.
- Git graph.
- Syntactic diff.
- Inline blame.
- DAP debugging.
- Test runner.
- Coverage gutters.
- Dependency/security panels.

This prevents Legion from being “just another AI wrapper.”

## 3. Dock system design

### 3.1 Core types

Create front-end/UI types equivalent to:

```rust
pub enum DockSide {
    Left,
    Right,
    Bottom,
}

pub enum PanelCapability {
    Deterministic,
    RequiresAi,
    RequiresNetwork,
    RequiresCloud,
    RequiresDebugAdapter,
    RequiresTerminal,
    RequiresGit,
    RequiresLsp,
}

pub enum LegionMode {
    Manual,
    Assist,
    Delegate,
    Automate,
}

pub struct PanelMetadata {
    pub id: PanelId,
    pub title: String,
    pub icon: Option<String>,
    pub default_dock: DockSide,
    pub capabilities: Vec<PanelCapability>,
    pub default_visible: bool,
    pub minimum_mode: LegionMode,
}
```

Panel trait:

```rust
pub trait DockPanel {
    fn metadata(&self) -> PanelMetadata;
    fn render(&mut self, ui: &mut egui::Ui, ctx: &PanelCtx);
    fn persist_state(&self) -> serde_json::Value;
    fn restore_state(&mut self, value: serde_json::Value);
}
```

Registry:

```rust
pub struct PanelRegistry {
    panels: HashMap<PanelId, Box<dyn DockPanel>>,
}
```

Filtering:

```rust
impl PanelRegistry {
    pub fn visible_for(&self, mode: LegionMode, build_caps: BuildCapabilities) -> Vec<PanelId> {
        self.panels
            .iter()
            .filter(|(_, panel)| panel_allowed(panel.metadata(), mode, build_caps))
            .map(|(id, _)| *id)
            .collect()
    }
}
```

### 3.2 Panel filtering rules

Manual:

- allow deterministic panels.
- deny any panel with `RequiresAi`.
- deny any panel with `RequiresCloud`.
- deny any panel with `RequiresNetwork` unless explicitly marked Manual-safe and user-enabled.

Assist:

- allow deterministic panels.
- allow inline AI panels.
- deny autonomous/fleet panels.

Delegate:

- allow chat.
- allow codebase context panels.
- allow proposal/approval panels.
- deny fleet orchestration panels by default.

Automate:

- allow all panels subject to permission policy.

Offline build:

- compile without AI crates.
- compile without cloud-provider panels.
- registry cannot register AI panel constructors at all.

### 3.3 Dock layout persistence

Persist per mode:

```rust
pub struct DockLayout {
    pub mode: LegionMode,
    pub left: DockRegionLayout,
    pub right: DockRegionLayout,
    pub bottom: DockRegionLayout,
}

pub struct DockRegionLayout {
    pub collapsed: bool,
    pub splitter_fraction: f32,
    pub pinned_default: Option<PanelId>,
    pub panels: Vec<PanelId>,
    pub active_panel: Option<PanelId>,
}
```

Storage keys:

```text
layout/manual/left
layout/manual/right
layout/manual/bottom
layout/assist/left
layout/assist/right
layout/assist/bottom
layout/delegate/left
layout/delegate/right
layout/delegate/bottom
layout/automate/left
layout/automate/right
layout/automate/bottom
```

Migration rule:

- If old `RightConsole` layout exists, convert selected mode into right-dock active panel.
- If no layout exists, use mode defaults.

### 3.4 Dock rendering approach

Use egui/eframe side panels:

- `SidePanel::left("legion_left_dock")`
- `SidePanel::right("legion_right_dock")`
- `TopBottomPanel::bottom("legion_bottom_dock")`

Each dock region should contain:

1. header row
   - dock name
   - active panel tabs
   - collapse button
   - layout menu

2. pinned panel region
   - optional per mode
   - e.g. Project in Manual, Legion Board in Automate

3. custom toolkit region
   - tabbed or stacked secondary panels

4. resize handling
   - persist split fraction
   - clamp minimum widths/heights
   - test egui 0.34.2 clipping behavior

### 3.5 Front-end test requirements

Unit tests:

- Manual registry excludes AI panels.
- Offline build does not register AI constructors.
- Mode-specific layouts persist independently.
- Layout migration from old right console state succeeds.
- Unknown panel IDs in persisted layout are ignored safely.

Integration tests:

- Switch Manual → Automate → Manual and verify Manual does not show AI panels.
- Collapse/resize/reopen docks and verify persistence.
- Disable cloud capability and verify cloud panels disappear.
- Create fake AI panel and assert Manual cannot instantiate it.

## 4. Panel catalog

### 4.1 Manual-eligible panels

Left dock:

- Project Explorer.
- Symbol Outline.
- Git Status.
- Test Explorer.
- Structural Search saved patterns.

Right dock:

- Inspector.
- Call Hierarchy.
- Type Hierarchy.
- Dependency Inspector.
- Security Advisory Panel.
- Structural Search editor.
- Debug Variables/Watch.

Bottom dock:

- Terminal.
- Problems/Diagnostics.
- Search Results.
- Find References.
- Test Results.
- Coverage.
- Git Graph.
- Diff View.
- Debug Console.
- Agent logs are not Manual-eligible.

In-editor:

- Breadcrumbs.
- Sticky scroll.
- Inline diagnostics.
- Quick-fix lightbulbs.
- Inlay hints.
- Code lens.
- Inline blame.
- Coverage gutters.
- Breakpoints.
- Debug inline values.

### 4.2 Assist panels

Assist should feel like Manual plus predictive help.

Panels:

- Inline Prediction Settings.
- AI Action History.
- Local Model Status.
- Context Preview.

In-editor:

- Ghost text.
- Next edit prediction.
- Inline rewrite preview.
- Accept/reject shortcuts.

Do not include:

- chat by default.
- autonomous task board.
- worker lanes.

### 4.3 Delegate panels

Panels:

- Chat.
- Context Inspector.
- Prompt/Request Builder.
- Proposal Queue.
- Multi-file Diff Review.
- Tool Permission Queue.
- Model Route Inspector.

Key UX:

- Every model-generated edit appears as a proposal.
- Proposals can be accepted/rejected per file and per hunk.
- User sees exact context sent to model.
- User sees exact tool calls requested.
- Tool permission decisions are visible and revocable.

### 4.4 Automate panels

Automate is where Legion becomes distinctive.

Panels:

- Legion Board.
- Fleet Console.
- Worker Lane Status.
- Worker Detail.
- Decision Feed.
- Risk Monitor.
- Cloud Lane Usage.
- Validation Matrix.
- Conflict Graph.
- Merge Readiness.
- Kill Switch.
- Audit Trail.

Legion Board columns:

- Intake.
- Planned.
- Ready.
- Running.
- Validating.
- Needs Review.
- Blocked.
- Accepted.
- Rejected.
- Killed.

Each card shows:

- task title.
- role assigned.
- scope.
- allowed files.
- validation command.
- current worker.
- risk level.
- evidence status.
- conflicts.
- retry count.
- elapsed time.

Worker detail shows:

- worker ID.
- role.
- model.
- provider.
- local/cloud.
- prompt/task packet hash.
- files allowed.
- files touched.
- output schema.
- validation result.
- termination reason.

Decision feed shows:

- planner decomposition.
- routing decisions.
- context selection.
- worker spawn.
- patch proposal.
- validation output.
- conflict detection.
- reviewer decision.
- user approval.
- cleanup.

Risk monitor shows:

- high-risk files.
- secret patterns.
- auth/security changes.
- build-system changes.
- generated binary files.
- tool permission escalation.
- repeated failures.
- out-of-scope diff.

## 5. Proposal/evidence UX

### 5.1 Proposal review surface

Each proposal should include:

- Summary.
- Rationale.
- Changed files.
- Diff.
- Validation commands run.
- Validation output.
- Risk flags.
- Conflicts.
- Worker provenance.
- Accept/reject controls.

### 5.2 Review states

Proposal states:

- Draft.
- Validating.
- ValidationFailed.
- ValidationPassed.
- NeedsHumanReview.
- Accepted.
- Rejected.
- Superseded.
- Applied.

### 5.3 Per-hunk controls

Controls:

- accept hunk.
- reject hunk.
- accept file.
- reject file.
- request revision.
- escalate to stronger model.
- open related evidence.

### 5.4 Evidence-first display

Every Automate result should be evidence-backed.

Bad UI:

```text
Worker says fix is done.
```

Good UI:

```text
Patch proposed by rust-compiler-fixer-3b.
Allowed files: crates/devil-agent/src/lib.rs only.
Touched files: crates/devil-agent/src/lib.rs only.
Validation:
  cargo check -p devil-agent: passed.
  cargo test -p devil-agent: passed.
Risk: low.
Conflicts: none.
```

## 6. Legion cloud UX

Cloud UX should be visible but not overwhelming.

Add a `Cloud Lanes` panel in Automate mode:

- available local lanes.
- available cloud lanes.
- queue depth.
- current monthly usage.
- lane-minute estimate.
- cost guardrails.
- current cloud sync scope.
- uploaded files for current task.

Before sending to cloud, user should be able to inspect:

- task objective.
- files/snippets included.
- forbidden paths.
- secrets scan result.
- model/provider route.
- expected cost estimate.

Cloud permission levels:

1. Never use cloud.
2. Ask every time.
3. Allow deterministic validation only.
4. Allow low-risk AI worker tasks.
5. Allow configured project cloud policy.

## 7. Front-end rename plan

### 7.1 Immediate user-facing rename

Change:

- app window title.
- splash screen.
- about dialog.
- docs.
- menus.
- command palette prefixes.
- mode names if they include Devil.
- icons/branding assets.

Keep temporarily:

- crate names.
- internal module names.
- old planning phase paths.

### 7.2 Controlled internal rename

Later migrate:

- `devil-ui` → `legion-ui`.
- `devil-agent` → `legion-agent`.
- `devil-ai` → `legion-ai`.
- `devil-ai-providers` → `legion-ai-providers`.
- `devil-protocol` → `legion-protocol`.
- `devil-app` → `legion-app`.

Do it only after the dock refactor is stable.

## 8. Front-end implementation sequence

### Phase FE-1: Panel registry foundation

Tasks:

1. Define `PanelId` enum.
2. Define `DockSide` enum.
3. Define `LegionMode` enum if not already present.
4. Define `PanelCapability` enum.
5. Define `PanelMetadata`.
6. Define `DockPanel` trait.
7. Define `PanelRegistry`.
8. Implement mode filtering.
9. Implement build capability filtering.
10. Add tests for filtering.

Exit criteria:

- Registry can register deterministic and AI panels.
- Manual returns deterministic panels only.
- Offline build can omit AI registrations.

### Phase FE-2: Dock host rendering

Tasks:

1. Create left/right/bottom dock host components.
2. Add tab/header rendering.
3. Add collapse behavior.
4. Add resize behavior.
5. Add persistence hooks.
6. Replace `RightConsole` wholesale swap.
7. Add mode-scoped default layouts.
8. Add old layout migration.

Exit criteria:

- Four modes render from shared registry.
- Layouts persist per mode.
- No AI panels appear in Manual.

### Phase FE-3: Manual deterministic panels

Tasks:

1. Outline panel.
2. Problems panel.
3. Search/results panel.
4. Quick-fix UI.
5. Breadcrumbs/sticky scope headers.
6. Git status panel.
7. Terminal panel.

Exit criteria:

- Manual mode is useful and stable.

### Phase FE-4: Structural tools panels

Tasks:

1. Structural Search panel.
2. Pattern editor.
3. Results preview.
4. Rewrite preview.
5. Apply proposal path.

Exit criteria:

- Structural Search & Replace works offline.

### Phase FE-5: VCS/debug/test panels

Tasks:

1. Git graph.
2. Syntactic diff.
3. Inline blame.
4. DAP debug panels.
5. Test explorer.
6. Coverage gutters.

Exit criteria:

- Legion feels like a real IDE before AI.

### Phase FE-6: Assist UI

Tasks:

1. Ghost text rendering.
2. Accept/reject commands.
3. AI action history.
4. Model status panel.
5. Context preview.

Exit criteria:

- Assist mode has AI help but no autonomous tool execution.

### Phase FE-7: Delegate UI

Tasks:

1. Chat panel.
2. Context inspector.
3. Proposal queue.
4. Multi-file diff review.
5. Tool permission queue.
6. Route inspector.

Exit criteria:

- AI proposes changes; user/app approves.

### Phase FE-8: Automate UI

Tasks:

1. Legion Board.
2. Fleet Console.
3. Worker detail.
4. Decision feed.
5. Risk monitor.
6. Cloud lane usage.
7. Validation matrix.
8. Conflict graph.
9. Kill switch.

Exit criteria:

- User can watch, control, validate, and kill a multi-worker workflow.

## 9. Front-end quality gates

Before shipping Automate:

- Manual AI-exclusion test passes.
- Offline feature build passes.
- Proposal review cannot be bypassed from UI.
- Kill switch works from every Automate panel.
- Cloud upload scope is visible before send.
- Worker outputs display evidence.
- Risk flags are visible before approval.
- Per-hunk review works.
- Layouts persist without corrupting state.

## 10. Front-end product language

Use `Legion` branding consistently:

- Legion Board.
- Worker Lane.
- Fleet Console.
- Decision Feed.
- Risk Monitor.
- Proposal Queue.
- Evidence Ledger.
- Cloud Lane.
- Local Lane.
- Specialist.
- Disposable Worker.

Avoid overusing “agent” in the UI. “Worker lane” is clearer and less scary.

## 11. Front-end immediate next actions

1. Implement dock registry.
2. Add Manual filtering tests.
3. Replace `RightConsole` with dock hosts.
4. Persist per-mode layout.
5. Ship Outline + Problems as first registry panels.
6. Rename user-facing strings to Legion.
7. Delay internal crate rename until architecture stabilizes.
