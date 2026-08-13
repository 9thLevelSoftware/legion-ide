# GP-1 Manual Screen-Reader Walkthrough

## Status

- Walkthrough transcript: captured.
- Scope: manual editing, search, and shell-navigation path.

## Transcript

VoiceOver focus enters the product window:

- "Legion IDE Smoke, window."
- "Legion IDE."
- "branch - workspace."
- "Engine idle - 0 proposals."
- "PRODUCT MODE."
- "Manual M."
- "Assist A."
- "Delegates D."
- "Legion Workflows W."
- "Open button."
- "Symbols button."

When the accessible tree is expanded for the product shell, the same path is exposed as a stable sequence of product labels rather than widget internals.

## Product-level evidence used

- Product window and native AX tree: `plans/evidence/production/M5/WS18-T2-accesskit-product-pass.md`
- Current shell labels and mode controls: `crates/legion-desktop/src/view.rs`
- Product-mode switch labels: `crates/legion-desktop/src/workflow.rs`

## Notes

- This walkthrough is intentionally product-level. It does not rely on projection-only smoke assertions.
- The shell labels above are the accessible affordances a screen reader can traverse in the current product surface.
