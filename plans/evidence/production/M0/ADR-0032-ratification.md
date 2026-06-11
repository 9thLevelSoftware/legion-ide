# M0 — ADR-0032 (Editor Render Path) Ratification Evidence

Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
ADR: [`plans/adrs/ADR-0032-editor-render-path.md`](../../../adrs/ADR-0032-editor-render-path.md)
Date: 2026-06-10
Gate: `cargo run -p xtask -- no-egui-textedit` (WS01.T1 / ADR-0032 enforcement)
Acceptance target: master-plan §6 row "ADR-0032 | Editor render path" → option (a) ratified in-repo, with the renderer kept behind the existing projection boundary.

## Re-verification (post docs-hygiene fix)

The relative ADR link in the "ADR:" bullet above is three levels up
(`../../../adrs/…`) because this evidence file lives at
`plans/evidence/production/M0/`; the earlier `../../adrs/…` link was a
docs-hygiene violation caught by `cargo run -p xtask -- docs-hygiene`. After
the fix, all M0-relevant gates pass cleanly against the current working tree
(commit baseline `b56dcb2`, ratification changes untracked as required by the
task's "no commit without explicit user instruction" rule):

- `cargo run -p xtask -- no-egui-textedit` → `no-egui-textedit checks passed` (exit 0)
- `cargo test -p xtask --test no_egui_textedit` → `test result: ok. 6 passed; 0 failed` (exit 0)
- `cargo run -p xtask -- check-deps` → `dependency policy checks passed` (exit 0)
- `cargo run -p xtask -- docs-hygiene` → `documentation hygiene checks passed` (exit 0)

## Decision Recorded

- Status flipped from `Draft` to `Accepted` in `plans/adrs/ADR-0032-editor-render-path.md`.
- Decision text matches Production Master Plan v0.1 §6 recommendation verbatim (custom egui code-canvas widget; no `egui::TextEdit` in the code canvas; renderer stays behind the projection boundary so GPUI remains a live fallback re-evaluated every ~6 months per risk-register R1).
- No amendments were required. The ADR adds two minor confirmations consistent with the master plan:
  1. The `CodeCanvasPainter` seam is the renderer-portability boundary (decision section).
  2. The `no-egui-textedit` gate is the M0/WS01.T1 enforcement mechanism (verification section).

## Crate / Dependency Boundary Impact

- No new internal crate edges are introduced by this ADR.
- `legion-desktop` remains the only renderer crate allowed to host `egui`/`eframe` per `plans/dependency-policy.md` §1 (Directional Intent, "Renderer crates are adapter-only").
- `xtask` adds a `no-egui-textedit` subcommand (CLI gate) wired to `xtask/no-egui-textedit.toml` and the library in `xtask/src/no_egui_textedit.rs`. This is a build-time guardrail only; it does not affect any production crate boundary.
- The painter module is currently at `crates/legion-desktop/src/view/code_canvas_painter.rs` (mirrors the `crates/legion-desktop/src/view.rs` parent + painter split recommended by the ADR's decision section). The `scanned_paths` list in `xtask/no-egui-textedit.toml` will expand as the surface is split further (per the decision: "can expand as that surface is split into dedicated painter modules").

## Gate Evidence (verbatim)

### `cargo run -p xtask -- no-egui-textedit`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- no-egui-textedit
   Compiling xtask v0.1.0 (/Users/christopherwilloughby/legion-ide/xtask)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.87s
     Running `target/debug/xtask no-egui-textedit`
no-egui-textedit checks passed
```

Exit code: `0`. 0 violations across `crates/legion-desktop/src/view.rs` and `crates/legion-desktop/src/view/code_canvas_painter.rs`.

### `cargo test -p xtask --test no_egui_textedit`

```
running 6 tests
test no_egui_textedit_loads_config_from_toml ... ok
test no_egui_textedit_flags_textedit_in_scanned_path ... ok
test no_egui_textedit_allows_textedit_in_unscanned_path ... ok
test no_egui_textedit_allows_textedit_in_allowlisted_path ... ok
test no_egui_textedit_flags_textedit_in_painter_module ... ok
test no_egui_textedit_ignores_legion_text_textedit ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Companion gates (no regression)

- `cargo run -p xtask -- check-deps` → `dependency policy checks passed` (exit 0). Confirms `xtask` adding the new subcommand did not introduce any forbidden internal edge in `plans/dependency-policy.md`.
- `cargo run -p xtask -- docs-hygiene` → `documentation hygiene checks passed` (exit 0). Confirms the renamed ADR and M0 evidence package do not break doc-hygiene invariants.

## Invariant Preservation Checklist

- [x] Projection-only UI: `legion-ui` still emits `CommandDispatchIntent` and accepts snapshots; the code-canvas painter lives in `legion-desktop` (adapter crate), not in `legion-ui`. Unchanged.
- [x] Proposal-mediated mutation: unaffected. This ADR only governs the render path of the code canvas; save authority, snapshot leases, and proposal routing are unchanged.
- [x] Metadata-first observability: unaffected. The `no-egui-textedit` gate is a build-time lint, not a runtime sink; observability policy still rejects zero `CorrelationId` / nil `CausalityId` / zero `EventSequence`.
- [x] Fail-closed policy: enforced by the gate itself — any future re-introduction of `egui::TextEdit` in the scanned painter paths fails the gate at the same place the saver rejects a stale/conflict/denial outcome.

## Operational Notes

- The M0 ratification does **not** commit anything; the user retains explicit commit authority per the task body rule. The ADR flip and the new evidence package are working-tree changes only.
- The full workspace test surface (`cargo test --workspace --all-targets`) and other narrow gates (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`) are recorded at the milestone-claim level, not per ADR, and remain a prerequisite for the next phase-gate flip.
- WS01.T1 acceptance criteria ("gate exists and passes; canvas renders via custom painter only") are fully met by the evidence above.
