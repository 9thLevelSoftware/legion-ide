# Phase 1 Review: Baseline Reconciliation and Renderer Decision

## Result: PASSED

**Cycles Used**: 2
**Review Date**: 2026-05-26
**Reviewers Applied**: testing-qa-verification-specialist, testing-test-results-analyzer, engineering-senior-developer, engineering-security-engineer
**Adapter Note**: Codex CLI has no structured AskUserQuestion tool in this runtime, so reviewer confirmation was skipped for the user's direct `$legion review 1` command.

## Scope Reviewed

- `plans/phase-status-ledger.md`
- `plans/evidence/gui-productization/gui-productization-baseline.md`
- `plans/evidence/gui-productization/renderer-decision-matrix.md`
- `plans/adrs/ADR-0002-ui-editor-rendering.md`
- `plans/adrs/ADR-0030-desktop-adapter-boundary.md`
- `plans/desktop-adapter-boundary-v0.1.md`
- `plans/dependency-policy.md`
- `plans/evidence/gui-productization/phase-1-renderer-readiness.md`
- `xtask/src/main.rs`
- Phase 1 planning artifacts under `.planning/phases/01-baseline-reconciliation-and-renderer-decision/`

## Cycle 1 Findings

### Finding 1

- **File**: `xtask/src/main.rs`
- **Line/Section**: pre-review `run_check_deps` renderer gate call and `validate_renderer_dependency_gate`
- **Severity**: BLOCKER
- **Issue**: The renderer dependency gate enforced the deny list only for `devil-ui`, while the Phase 1 spec and policy require renderer crates to stay out of `devil-ui` and core/non-adapter workspace crates.
- **Details**: Plan 01-04 explicitly required enforcement so renderer dependencies cannot enter `devil-ui` or core crates. The policy also says renderer crates are adapter-only and must not appear in core substrate crates, but the runtime check passed only `devil-ui` dependency names into the gate. A package like `devil-app` could have declared `eframe` without `cargo run -p xtask -- check-deps` catching it.
- **Suggested Fix**: Build a dependency-name map for all workspace packages, skip only the planned `devil-desktop` adapter, and report renderer/windowing dependencies from any other package.
- **Confidence**: HIGH - 95%
- **Resolution**: Fixed in review cycle 1. `xtask` now validates all workspace packages except `devil-desktop`; the exact regression test covers both `devil-ui` and a non-UI core package.

### Finding 2

- **File**: Phase 1 plan/context artifacts
- **Line/Section**: EOF
- **Severity**: WARNING
- **Issue**: `git show --check af48d44` reported blank lines at EOF in five plan files and the phase context file.
- **Details**: This did not affect runtime behavior, but it contradicted the phase's evidence hygiene and made the committed phase artifact fail a whitespace check against the previous commit. The files were planning artifacts, not source code, so this was treated as cleanup rather than a functional blocker.
- **Suggested Fix**: Remove the trailing blank line at EOF from the affected Phase 1 planning files.
- **Confidence**: HIGH - 90%
- **Resolution**: Fixed in review cycle 1.

## Cycle 2 Result

The re-review passed after the enforcement fix and whitespace cleanup.

## External Source Spot Check

The renderer decision matrix remains consistent with current primary sources checked during review:

- egui's official README states that `eframe` supports native Windows and that official integrations include `egui-winit` plus `egui-wgpu` or `egui_glow`: https://github.com/emilk/egui
- eframe docs list default `accesskit`, `winit`, and `wgpu` feature paths: https://docs.rs/crate/eframe/latest/features
- AccessKit docs list egui and Slint integrations and recommend the AccessKit winit adapter for winit-based Rust toolkits: https://accesskit.dev/

## Verification

| Command | Result |
| --- | --- |
| `cargo test -p xtask renderer_dependency_gate_preserves_projection_boundary -- --exact` | passed, 1 passed |
| `cargo test -p xtask` | passed, 40 passed |
| `cargo run -p xtask -- check-deps` | passed |
| `cargo fmt --all --check` | passed |
| `cargo check --workspace --all-targets` | passed |
| `cargo test --workspace --all-targets` | passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | passed |
| `cargo deny check` | passed with warning-level duplicate-crate warnings |
| `git diff --check` | passed; line-ending conversion warnings only |
| Phase 1 plan artifact `rg` and size checks | passed |

## Residual Risk

- `cargo deny check` still reports duplicate-crate warnings under the repository's warning-level policy. This is not introduced by Phase 1 review work, but it remains dependency-health noise to track later.
- Phase 2 still needs real renderer evidence for input-to-paint, IME, clipboard, focus, high-DPI, file dialogs, and accessibility before GUI acceptance can be claimed.

## Post-Review Polish

No separate behavior-preserving code-polish pass was needed beyond the review fix. Cleanup was limited to the renderer gate scope, evidence wording, and trailing blank lines in Phase 1 planning files. The full verification set passed afterward.

## Verdict

**PASS** - Phase 1 is approved after one blocker and one warning were resolved.
