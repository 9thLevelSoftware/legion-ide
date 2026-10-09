# ADR-0032: Editor Render Path

## Status

Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.

This ADR ratifies the Production Master Plan v0.1 §6 recommendation verbatim
(option (a), custom egui code-canvas widget with the renderer kept behind the
existing projection boundary). The renderer boundary is enforced by the
`cargo run -p xtask -- no-egui-textedit` gate, which is recorded in
`plans/evidence/production/M0/` together with the supporting unit tests in
`xtask/tests/no_egui_textedit.rs`. No amendments to the master-plan
recommendation were required; GPUI / Slint remain live fallbacks to be
re-evaluated semi-annually per the plan's risk-register R1 mitigation.

## Context

The Production Master Plan defines WS-01 as the first critical-path workstream for a credible manual editor. The current renderer is an egui/eframe projection shell with theme/token plumbing, but the code canvas must not regress into an `egui::TextEdit`-owned editor because that would violate the projection-only UI boundary and make editor portability, virtualization, IME handling, and large-file safety harder to enforce.

ADR-0030 keeps the desktop adapter behind a projection boundary. This ADR chooses the concrete render-path strategy for M1 while preserving the ability to re-evaluate GPUI or another renderer later.

## Decision

Legion will continue with a custom egui code-canvas widget for M1. The code canvas owns painting and input translation only: shaped visible lines, gutters, selections, cursors, decorations, IME composition, and command intents. It does not own editor sessions, workspace state, save authority, or mutation authority.

`egui::TextEdit` is forbidden in the desktop code-canvas/editor render path. A repository gate (`cargo run -p xtask -- no-egui-textedit`) enforces this boundary for the current code-canvas module (`crates/legion-desktop/src/view.rs`) and can expand as that surface is split into dedicated painter modules.

The custom painter should introduce a `CodeCanvasPainter` seam so the editor remains renderer-portable. GPUI remains a live fallback to re-evaluate after M1 evidence, not a dependency to add during M0.

### Ticket 004 bounded accessibility projection

The coordinator approved complete small-buffer accessibility on 2026-10-09.
The desktop adapter may publish a stable text-input control and bounded
`TextRun` descendants from the existing exact `small_buffer_preview`, using
app-produced snapshot identity, editability, coverage and directed-caret
metadata. The renderer caches presentation data only; it gains no editor,
save, or filesystem authority. Its direct workspace-pinned Unicode
segmentation dependency describes accessibility units, and its dev-only
AccessKit consumer verifies the rendered contract. ADR-0015's large/degraded
whole-source restriction remains absolute: unavailable or over-budget source
publishes constrained coverage, never a complete viewport fragment. This
amendment records the approved scope; native TextPattern and native action
qualification remain external gates. See
`plans/evidence/ide-2026-ticket004-editor-accessibility-prerequisite.md`.

## Consequences

- **Positive:** preserves projection-only UI and enables row virtualization, per-line shaping caches, IME handling, gutter lanes, multibuffer review, and large-file degradation without fighting a general-purpose text widget.
- **Negative:** Legion must implement more editor behavior itself before M1 can feel credible.
- **Mitigation:** keep the painter seam narrow and validate with performance/accessibility gates before dogfooding.

## Verification

- `cargo run -p xtask -- no-egui-textedit`
- `cargo test -p xtask --test no_egui_textedit`
- M1 editor evidence under `plans/evidence/production/m1/` once the render path is product-validated.
