# AccessKit Product Pass and GP Screen-Reader Walkthroughs

## Status

- Product-level accessibility evidence: passed.
- Companion GP walkthrough transcripts: captured for GP-1, GP-2, and GP-3.

## Purpose

Record the product-level accessibility pass for the Legion desktop shell and the screen-reader walkthroughs for the three golden paths (GP-1..GP-3).

This evidence is product-facing. It is not limited to projection-only smoke.

## Source evidence

- OS accessibility-tree inspection for the product window: `plans/evidence/production/M5/WS18-T2-accesskit-product-pass.md`
- Product shell labels from the desktop view surfaces in `crates/legion-desktop/src/view/*.rs`
- Accessibility projection coverage from `crates/legion-desktop/tests/accessibility.rs`

## Walkthrough transcripts

- `plans/evidence/accessibility/gp-1-manual-walkthrough.md`
- `plans/evidence/accessibility/gp-2-assist-walkthrough.md`
- `plans/evidence/accessibility/gp-3-delegate-walkthrough.md`

## Acceptance note

The product pass is considered complete only when the product window is observable via the OS accessibility tree and the GP walkthroughs each include a transcript of the accessible surface that a screen reader can traverse.
