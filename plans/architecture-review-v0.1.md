# Legion IDE - Founding Architecture Review v0.1

Status: **PASS WITH CHANGES**

## Outcome

The proposed charter is directionally correct for an IDE-first, local-first, deterministic AI product, but it is not implementation-ready. The highest priority is to freeze the dependency direction, define explicit protocol boundaries, and create the missing Spike and freeze documentation before any active implementation.

## Required user-specified corrective action implemented

1. **`devil-ai` provider boundary inversion**
   - `devil-ai` now defines abstractions and request/response types and does not depend on `devil-ai-providers`.
   - `devil-ai-providers` now depends on `devil-ai` and is an adapter implementation crate.
   - This resolves the inversion risk and protects the core orchestrator from provider transport dependencies.

2. **Editor/Project coupling remains protocol-based**
   - No hard dependency from `devil-editor` to `devil-project` will be added.
   - The boundary is defined in `devil-protocol` as data-centric contract types + traits.

## Top findings and required edits

### 1. Implementation sequencing still over-commits before UI decision evidence
- **Finding:** `ADR-0002` is provisional pending Spike 1, yet `§17` has backend milestones chained directly after Step 4.
- **Evidence:** [`ADR-0002`](plans/adrs/ADR-0002-ui-editor-rendering.md:3-18), [`architecture-charter-v0.1 §17`](plans/architecture-charter-v0.1.md:950-968)
- **Edit:** Make Spike 1A branching explicit and block downstream steps until UI path and fallback are accepted in the freeze.

### 2. Early crate proliferation without proof fences
- **Finding:** The workspace is established with all founding crates before the first viability gates.
- **Evidence:** [`Cargo.toml`](Cargo.toml:3-21)
- **Edit:** Mark implementation scope clearly as Spike 1A-only and treat all non-essential crates as placeholders.

### 3. Provider boundary was reversed but now corrected
- **Finding:** Previously inverted dependency between `devil-ai` and `devil-ai-providers`.
- **Evidence:** Previous manifest state in [`crates/devil-ai/Cargo.toml`](crates/devil-ai/Cargo.toml:8-13) and corrected state now
- **Edit:** Completed by dependency inversion in `devil-ai` and `devil-ai-providers` manifests.

### 4. Missing protocol-boundary contract for editor/project context
- **Finding:** Charter and code show conceptual flow `Editor → Project`, but no formal protocol boundary existed.
- **Evidence:** [`§2.2`](plans/architecture-charter-v0.1.md:126-151), previous `crates/devil-editor/Cargo.toml`
- **Edit:** Add explicit project-related contract primitives to `devil-protocol` so editor can consume immutable project context without direct coupling.

### 5. No automated dependency-direction and protocol contract gates before freeze
- **Finding:** Validation gates cover runtime and privacy, but not dependency graph and contract-stability gates.
- **Evidence:** [`§16`](plans/architecture-charter-v0.1.md:890-948)
- **Edit:** Add dependency-direction and protocol-contract gates and enforce them in `architecture-freeze`.

### 6. Text model needs stress proof before indexing and AI integration
- **Finding:** Text/snapshot performance constraints are not currently measurable in the gating criteria.
- **Evidence:** [`§16.2`](plans/architecture-charter-v0.1.md:904-914), [`ADR-0003`](plans/adrs/ADR-0003-editor-core-text-model.md:9-16)
- **Edit:** Add explicit large-file and memory-budgeting validation for spike handoff.

### 7. Platform scope is still overloaded in text
- **Finding:** `devil-platform` still appears to include editor/window integration concerns.
- **Evidence:** [`§3.2`](plans/architecture-charter-v0.1.md:235-253), [`§1.5`](plans/architecture-charter-v0.1.md:58-65)
- **Edit:** Bound `devil-platform` to OS services and create dedicated platform boundary proof before Spike 1A.

### 8. Planning artifacts for architecture freeze are still missing
- **Finding:** Several required planning documents were empty placeholders.
- **Evidence:** `plans/architecture-freeze-v0.1.md`, `plans/milestone-0-feasibility-proofs.md`, `plans/SPIKE-001A-native-shell-proof.md`
- **Edit:** Populate all planning/spike docs before implementation.

## Required edits before Spike 1A (final)

1. Apply AI/provider dependency inversion in manifests.
2. Add protocol boundary contract for editor/project interaction in `devil-protocol`.
3. Add dependency-direction validation and protocol-contract gates in charter + freeze criteria.
4. Add text-model stress gate in validation section.
5. Add `SPIKE-000-platform-boundary-proof.md` and define platform contract boundaries.
6. Populate and review:
   - `plans/architecture-freeze-v0.1.md`
   - `plans/milestone-0-feasibility-proofs.md`
   - `plans/SPIKE-001A-native-shell-proof.md`
7. Keep these crates placeholder-only for Spike 1A: `devil-agent`, `devil-memory`, `devil-ai-providers` (adapters), `devil-cli`, `devil-observability`.
8. Block any implementation progress until gates and placeholders are confirmed.

## Final recommendation

Approve with changes and hold implementation. The charter can be frozen only after the listed edits and Gate proofs are completed.

