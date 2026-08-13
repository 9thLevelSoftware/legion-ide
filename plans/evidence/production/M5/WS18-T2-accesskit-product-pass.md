# M5 — WS18.T2 AccessKit Product Pass Evidence

## Status

Verified for OS accessibility-tree inspection evidence.

## Acceptance target

- Confirm the Legion desktop shell publishes an OS accessibility tree for the product window.
- Record the observable tree shape for the active product shell so the known limitation is closed.
- Preserve the existing smoke evidence without claiming scripted NVDA/VoiceOver/Orca automation where it was not feasible on this runner.

## What was verified

- `cargo run -p legion-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 60000 --evidence plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md --session-state target/gui-phase6-session.json --diagnostics-export target/gui-phase6-diagnostics.md`
  - Passed.
  - The timed smoke run still reports the projected accessibility node count and the renderer smoke metrics.
- Swift Accessibility API inspection against the running `legion-desktop` smoke process.
  - `AXIsProcessTrusted() = true`
  - `AXApplication` title: `legion-desktop`
  - Focused window title: `Legion IDE Smoke`
  - Window role: `AXWindow`
  - Window subrole: `AXStandardWindow`
  - Window child count: `5`
  - Child roles observed at depth 2 included `AXGroup`, `AXButton`, and `AXStaticText`
  - The top-level group exposed the product shell labels, including `Legion IDE`, `branch - workspace`, `Engine idle - 0 proposals`, `PRODUCT MODE`, and the product-mode labels `Manual M`, `Assist A`, `Delegates D`, and `Legion Workflows W`
  - Window controls were also exposed (`AXCloseButton`, `AXFullScreenButton`, `AXMinimizeButton`)

## Command excerpts

### Smoke run

```text
cargo run -p legion-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 60000 --evidence plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md --session-state target/gui-phase6-session.json --diagnostics-export target/gui-phase6-diagnostics.md
```

### Accessibility tree inspection

```text
trusted=true
focusedErr=0
AXWindow subrole=AXStandardWindow title=Legion IDE Smoke
  [0] AXGroup subrole=AXUnknown
  [0]   [0] AXStaticText subrole=AXUnknown value=Legion IDE
  [0]   [1] AXStaticText subrole=AXUnknown value=Legion IDE
  [0]   [2] AXStaticText subrole=AXUnknown value=branch - workspace
  [0]   [3] AXStaticText subrole=AXUnknown value=Engine idle - 0 proposals
  [0]   [4] AXStaticText subrole=AXUnknown value=PRODUCT MODE
  [0]   [5] AXStaticText subrole=AXUnknown value=Manual M
  [0]   [6] AXStaticText subrole=AXUnknown value=Assist A
  [0]   [7] AXStaticText subrole=AXUnknown value=Delegates D
  [0]   [8] AXStaticText subrole=AXUnknown value=Legion Workflows W
  [0]   [9] AXStaticText subrole=AXUnknown value=MK
  [0]   [10] AXButton subrole=AXUnknown title=Open
  [0]   [11] AXButton subrole=AXUnknown title=Symbols
  [1] AXButton subrole=AXCloseButton
  [2] AXButton subrole=AXFullScreenButton
  [3] AXButton subrole=AXMinimizeButton
  [4] AXStaticText value=Legion IDE Smoke
```

## Residual risk

- Screen-reader end-to-end automation for NVDA, VoiceOver, and Orca still needs dedicated host-specific runs where available.
- High-contrast, reduced-motion, and live-region behavior remain covered by product intent and existing projection/smoke scaffolding, but this task’s hard evidence is the OS accessibility-tree inspection above.
