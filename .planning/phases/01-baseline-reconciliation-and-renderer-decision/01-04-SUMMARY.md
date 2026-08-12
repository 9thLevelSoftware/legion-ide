# Plan 01-04 Summary: Dependency Policy And Xtask Renderer Gate

Status: Complete

## Files Changed

- `plans/dependency-policy.md`
- `xtask/src/main.rs`

## Verification Results

- `rg -q "devil-desktop" plans/dependency-policy.md`: passed
- `rg -q "renderer" plans/dependency-policy.md`: passed
- `rg -q "renderer_dependency_gate_preserves_projection_boundary" xtask/src/main.rs`: passed
- `cargo test -p xtask renderer_dependency_gate_preserves_projection_boundary -- --exact`: passed, 1 passed
- `cargo test -p xtask`: passed, 41 passed
- `cargo run -p xtask -- check-deps`: passed

## Decisions

- Renderer dependencies are adapter-only and permitted for `devil-desktop` after policy authorization.
- `devil-ui` now has a conservative renderer/windowing deny list enforced by `xtask`.

## Issues

- Initial exact test invocation matched zero tests due module-qualified naming. A root-level exact test now verifies the gate and the command passes.
