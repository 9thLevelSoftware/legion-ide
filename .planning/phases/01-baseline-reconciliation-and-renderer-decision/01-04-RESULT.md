Status: Complete
Plan: 01-04
Wave: 2
Agent: engineering-senior-developer, testing-qa-verification-specialist

Files:
- plans/dependency-policy.md
- xtask/src/main.rs

Verification:
- rg -q "devil-desktop" plans/dependency-policy.md: passed
- rg -q "renderer" plans/dependency-policy.md: passed
- rg -q "renderer_dependency_gate_preserves_projection_boundary" xtask/src/main.rs: passed
- cargo test -p xtask renderer_dependency_gate_preserves_projection_boundary -- --exact: passed
- cargo test -p xtask: passed, 41 passed
- cargo run -p xtask -- check-deps: passed

Decisions:
- Added adapter-only renderer dependency policy for `devil-desktop`.
- Added a fail-closed `xtask` renderer dependency gate for `devil-ui` and policy-boundary markers.

Issues:
- The first exact test run matched zero tests because the test was nested under `tests::`; corrected by adding the exact root test and reran successfully.

Errors:
- None remaining.
