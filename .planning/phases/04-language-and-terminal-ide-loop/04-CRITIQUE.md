# Phase 4 Plan Critique -- Language and Terminal IDE Loop

## Verdict

PASS after auto-refine.

## Critique Findings And Refinements

1. Legacy governance collision:
   - Finding: `plans/dependency-policy.md` and `xtask/src/main.rs` still use the older Phase 4 agentic AI gate, while the active GUI roadmap uses Phase 4 for language and terminal workflows.
   - Refinement: Plan 04-01 now makes this the first implementation task and requires compatibility-preserving policy/xtask treatment before app dependency edges are added.

2. Boundary ordering:
   - Finding: Implementing app language or terminal behavior before protocol/UI projection contracts would force later agents to infer DTOs.
   - Refinement: Plan 04-01 owns protocol/UI projection contracts; Plans 04-02 and 04-03 depend on it.

3. Edit-producing language actions:
   - Finding: Formatting, rename, organize imports, and code actions are high-risk because they can appear to be editor commands.
   - Refinement: Plan 04-02 requires proposal previews, tests for unchanged editor/disk state before proposal acceptance, and `BLOCKED` if proposal conversion cannot represent a required path.

4. Terminal security and output bounds:
   - Finding: The terminal runtime exists but is default-disabled and policy-gated; desktop must not get direct runtime authority.
   - Refinement: Plan 04-03 routes all terminal lifecycle through app/security authority and requires visible denial/error projections plus bounded output/search status.

5. Desktop projection-only rule:
   - Finding: Desktop panel work can accidentally import runtime crates or copy app state.
   - Refinement: Plan 04-04 forbids runtime/security/index/editor/project/storage imports and requires actions to dispatch `CommandDispatchIntent` only.

6. Acceptance evidence:
   - Finding: Roadmap criteria span product behavior, safety boundaries, and full repository gates.
   - Refinement: Plan 04-05 adds targeted boundary tests and safety evidence; Plan 04-06 owns final evidence and state updates only after full gates pass.

## Parallelization Decision

The phase remains sequential across six waves. Although language and terminal app workflows touch different conceptual surfaces, both depend on Plan 04-01 policy/projection work and both modify `crates/devil-app/src/lib.rs`. Desktop and acceptance work must wait for app projections and command routing. Sequential waves reduce file-conflict and boundary-regression risk.

## Residual Risks For Build

- `xtask check-deps` may require a careful compatibility branch because the old Phase 4 evidence path remains authoritative for historical gates.
- Existing protocol DTOs may be sufficient but large; build agents must reuse them rather than add parallel types.
- Native terminal behavior is host-dependent; tests should use deterministic fixture/runtime paths and project native unavailability explicitly.
- Broad `cargo test --workspace --all-targets` can be expensive and may expose unrelated failures; final acceptance remains blocked unless required gates pass or the project owner changes the gate.
