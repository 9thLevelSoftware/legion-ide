---
{"body":"Source: .planning/ROADMAP.md\n\n**Goal**: Establish the exact current state, reconcile planning truth, and choose a GUI renderer path without weakening architecture gates.\n\n**Requirements**: R-001, R-002, R-003, R-004\n\n**Recommended Agents**: project-manager-senior, engineering-senior-developer, testing-tool-evaluator, engineering-security-engineer\n\n**Success Criteria**:\n- Phase ledger/evidence conflict is resolved or explicitly superseded for the GUI track.\n- Renderer ADR records accepted stack, fallback criteria, and Windows-first evidence requirements.\n- Desktop adapter boundary is specified before code is added.\n- Dependency policy and `xtask` rules describe any approved renderer crate edges.\n- `legion-ui` remains projection-only and no GUI dependency is introduced without policy coverage.\n- Verification includes `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, and targeted app/UI tests.\n\n**Plans**: 7","changeId":"chg_phase-1-baseline-reconciliation-and-renderer-decision","dependencies":[{"mediaType":"text/markdown","path":".legion/project/specs/req_phase-1-baseline-reconciliation-and-renderer-decision.md","sha256":"sha256:a1ffb3475aa01ec5a56abf1e4dd201209a1c114e99dd3616d6aaca703a313894"}],"kind":"change-design","schemaVersion":"0.1.0","title":"Phase 1 implementation plan"}
---

# Phase 1 implementation plan

Source: .planning/ROADMAP.md

**Goal**: Establish the exact current state, reconcile planning truth, and choose a GUI renderer path without weakening architecture gates.

**Requirements**: R-001, R-002, R-003, R-004

**Recommended Agents**: project-manager-senior, engineering-senior-developer, testing-tool-evaluator, engineering-security-engineer

**Success Criteria**:
- Phase ledger/evidence conflict is resolved or explicitly superseded for the GUI track.
- Renderer ADR records accepted stack, fallback criteria, and Windows-first evidence requirements.
- Desktop adapter boundary is specified before code is added.
- Dependency policy and `xtask` rules describe any approved renderer crate edges.
- `legion-ui` remains projection-only and no GUI dependency is introduced without policy coverage.
- Verification includes `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, and targeted app/UI tests.

**Plans**: 7
