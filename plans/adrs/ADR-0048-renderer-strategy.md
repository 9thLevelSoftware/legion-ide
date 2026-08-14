# ADR-0048: Renderer Strategy — Stay on egui, Prove the Budgets

## Status

Accepted

## Context

Legion IDE uses egui 0.34.2 / eframe 0.34.2 with the default glow (OpenGL)
backend for its desktop renderer. ADR-0002 selected eframe/egui as the
Windows-first renderer-backed proof, keeping GPUI as an "architectural
influence" rather than a Phase 2 dependency because GPUI is pre-1.0 and
macOS/Linux-only. ADR-0032 ratified the custom code-canvas widget, banned
`egui::TextEdit` in the editor render path, established a `CodeCanvasPainter`
seam for renderer portability, and mandated semi-annual GPUI re-evaluation.

The 2026 IDE market shows GPU-native rendering as a competitive battleground:
Zed's GPUI targets Metal/DirectX with sub-2ms input latency. The course
correction plan (W1) flags this as the "single largest divergence between the
research and the build."

However, Legion already has meaningful infrastructure in place:

- A `CodeCanvasPainter` trait (`crates/legion-desktop/src/view/code_canvas_painter.rs:14`)
  provides a renderer-portable seam. Only one implementation exists today
  (`EguiCodeCanvasPainter`), but the trait is object-safe and deliberately
  designed for backend substitution.
- Custom text rendering uses the egui LayoutJob/Galley pipeline with a
  512-entry galley cache for shaped text reuse.
- `manual_perf.rs` defines real latency budgets enforced by a measurement
  harness: keypress p50 < 16 ms, keypress p95 < 32 ms, scroll p95 < 32 ms.
- The `no-egui-textedit` xtask gate prevents regression into a general-purpose
  text widget.

The renderer is the foundation layer. Migrating it mid-product is extremely
expensive — it touches text shaping, selection, IME, clipboard, focus
traversal, accessibility, high-DPI, and every desktop panel. The decision must
balance competitive ambition against delivery risk.

## Options Considered

### Option A: Stay on egui, prove the budgets

Keep egui 0.34.2 / eframe / glow (OpenGL). Invest in galley cache optimization,
batch rendering, selective repaint, and row virtualization to meet the existing
latency budgets. The `CodeCanvasPainter` seam remains the documented escape
hatch. Re-evaluate semi-annually per ADR-0032.

- **Risk**: immediate-mode redraw imposes a latency ceiling; egui's styling
  system limits visual polish compared to retained-mode renderers.
- **Benefit**: lowest risk to ship, no migration cost, Windows/macOS/Linux
  parity already working, budgets are testable with the existing harness.

### Option B: Switch to egui-wgpu backend

Swap eframe's glow backend for its wgpu backend. The egui API surface stays
identical — GPU-backed tessellation replaces OpenGL.

- **Risk**: wgpu maturity on Windows (DirectX 12 backend), potential regression
  in text rendering and startup time, additional GPU driver requirements for
  users.
- **Benefit**: GPU-backed rendering without a full framework migration;
  incremental upgrade path from Option A.

### Option C: Plan migration toward GPUI-class renderer

Build a custom retained-mode renderer on winit + wgpu + AccessKit, or adopt
GPUI directly once it reaches cross-platform maturity.

- **Risk**: 6-12 month migration; GPUI is pre-1.0 and macOS-only today; must
  rebuild text shaping, selection, IME, clipboard, focus traversal, and
  accessibility from scratch; no guarantee that the resulting renderer meets
  budgets faster than optimized egui.
- **Benefit**: maximum performance ceiling, full control over rendering
  pipeline, competitive parity with Zed's architecture.

## Decision

Option A, with Option B tracked as a future upgrade path.

The existing latency budgets (keypress p50 < 16 ms, p95 < 32 ms, scroll
p95 < 32 ms) need to be **proven** before a renderer migration is justified.
If the budgets are met on all supported platforms, the egui stack is sufficient
for daily-driver use and the competitive gap is narrower than it appears — users
experience latency budgets, not rendering architectures.

If the budgets are **not** met after optimization work (galley cache tuning,
selective repaint, batch rendering), the `CodeCanvasPainter` seam enables a
targeted migration to wgpu or a custom renderer without rewriting the entire
desktop adapter. Option B (egui-wgpu) is the first escalation step because it
preserves the egui API while changing the GPU backend.

Option C is premature. It is a 6-12 month migration with compounding risk:
GPUI lacks Windows/Linux parity, and building a custom retained-mode renderer
requires reimplementing text infrastructure that egui already provides. This
option remains a live fallback per ADR-0032's semi-annual re-evaluation, not a
planned commitment.

## Re-evaluation Triggers

- `manual_perf.rs` p95 consistently exceeds 32 ms on any supported OS after
  optimization work is complete.
- egui upstream drops support for a required platform or enters maintenance-only
  status.
- GPUI reaches 1.0 with Windows and Linux parity.
- A competitive threat (e.g., user-visible latency gap in direct comparison
  testing) makes the egui ceiling a product-blocking concern.

## Consequences

- Renderer investment goes to making egui fast enough — galley cache tuning,
  selective repaint, batch rendering, row virtualization — rather than a
  speculative rewrite.
- The `CodeCanvasPainter` seam is maintained and documented as the escape hatch.
  Any new rendering code must go through this trait, not bypass it.
- The `manual_perf.rs` harness and its budgets are the primary evidence gate
  for this decision. If the budgets are not met, this ADR's decision is
  revisited through the re-evaluation triggers above.
- Option B (egui-wgpu) is tracked as a low-risk upgrade path. A future ADR
  may accept it if GPU-backed tessellation proves beneficial without regression.

## References

- ADR-0002: original renderer selection (`plans/adrs/ADR-0002-ui-editor-rendering.md`)
- ADR-0032: code-canvas widget ratification (`plans/adrs/ADR-0032-editor-render-path.md`)
- `CodeCanvasPainter` trait: `crates/legion-desktop/src/view/code_canvas_painter.rs:14`
- `manual_perf.rs` budgets: `crates/legion-desktop/src/manual_perf.rs` — keypress p50 < 16 ms, keypress p95 < 32 ms, scroll p95 < 32 ms
- Course correction plan W1 finding: renderer divergence from market research
