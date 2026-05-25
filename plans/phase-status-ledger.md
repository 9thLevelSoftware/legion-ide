# Devil IDE Phase Status Ledger

Prepared: 2026-05-24  
Authority: [`plans/implementation-plan.md`](implementation-plan.md), [`plans/remaining-implementation-tasks-plan-v0.1.md`](remaining-implementation-tasks-plan-v0.1.md), and [`plans/evidence/`](evidence/)  
Purpose: unambiguous mapping of accepted evidence to each phase before implementation resumes.

---

## Phase summary

| Phase | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Phase 0 — Foundation and freeze | **Accepted** | [`plans/evidence/phase-0/`](evidence/phase-0/) | Architecture freeze recorded in [`plans/architecture-freeze-v0.1.md`](architecture-freeze-v0.1.md). Dependency policy enforced by `xtask`. |
| Phase 1 — Editor and text substrate | **Accepted** | [`plans/evidence/phase-1/editor-text-substrate.md`](evidence/phase-1/editor-text-substrate.md) | Degraded large-file mode, chunk descriptors, viewport projection, and bounded fake consumers are evidenced. 100MB full-cache boundary is measurement-only, not a green benchmark. |
| Phase 2 — Proposal mutation substrate | **Accepted** | [`plans/evidence/phase-2/proposal-mutation-substrate.md`](evidence/phase-2/proposal-mutation-substrate.md) | DTOs, routing, lifecycle states, deny-by-default validation, save apply, open-buffer text edit apply, closed-file create/delete/rename apply, multi-file workspace-edit execution, edit-only code-action execution, accepted reversible batch apply/rollback, workspace-authorized audit-failure rollback checkpoints, batch planning contracts, and live proposal ledger projection are accepted. Raw format execution and future runtime surfaces remain gated unless lowered into accepted proposal payloads. |
| Phase 3 — Semantic fabric and LSP supervision | **Accepted** | [`plans/evidence/phase-3/predictive-semantic-fabric.md`](evidence/phase-3/predictive-semantic-fabric.md) | `devil-index` is activated for actor-owned bounded scheduling, workspace-authored discovery import, descriptor/lease indexing, syntax-cache freshness, graph records, metadata-only persistence, semantic query APIs, and LSP supervision/proposal-routing DTOs. Vector indexing and later runtime surfaces remain deferred. |
| Phases 4–8 — AI, plugins, collaboration, remote, hardening | **Future-gated** | None accepted | Placeholder crates remain inert. Each phase requires its own ADR, dependency-policy entry, protocol contracts, contract tests, ownership tests, and evidence. |

---

## ADR status reconciliation

| ADR | Status | Blocker or reservation |
| --- | --- | --- |
| ADR-0001 — Rust workspace | Accepted | — |
| ADR-0002 — UI/editor rendering | **Accepted with reservations** | Renderer-backed p50/p95 input-to-paint, IME, clipboard, focus, and accessibility evidence are follow-ups. Spike 1A is accepted with reservations in [`plans/spikes/SPIKE-001A-result.md`](spikes/SPIKE-001A-result.md). |
| ADR-0003 — Editor core text model | Accepted | Large-file and retained-history benchmark reservations recorded in Phase 1 evidence. |
| ADR-0004 — Async runtime actor model | Accepted | — |
| ADR-0005 — Storage backends | **Accepted with reservations** | Spike 3 vector-store evaluation is deferred. SQLite/Tantivy metadata baseline is accepted; durable semantic/tracker/memory/plugin/collaboration/replay storage requires follow-up ADR. |
| ADR-0006 — AI provider abstraction | Accepted (governance-only) | No runtime behavior until Phase 4 gates. |
| ADR-0007 — Mode policy engine | Accepted | — |
| ADR-0008 — Tracker schema | Accepted (governance-only) | No runtime behavior until tracker activation gates. |
| ADR-0009 — Memory consent | Accepted (governance-only) | No runtime behavior until memory activation gates. |
| ADR-0010 — Air-gap mode | Accepted (governance-only) | No runtime behavior until AI/provider activation gates. |
| ADR-0015 — Streaming text viewport | Accepted | — |
| ADR-0016 — Generalized proposal service | Accepted | Phase 2 accepted for save, text edit, closed-file, workspace-edit, edit-only code-action, and reversible batch proposal routes. Future runtime apply surfaces remain separately gated. |
| ADR-0017 — Semantic fabric indexing | Accepted | Phase 3 evidence accepts the bounded semantic fabric runtime; vector indexing remains deferred. |
| ADR-0018 — LSP runtime supervision | Accepted | Phase 3 evidence accepts metadata-only LSP supervision contracts and proposal-routed edit outputs; command/process/runtime expansion remains separately gated. |

---

## Historical claim annotations

The following older architecture-review findings describe pre-rebaseline behavior and are **historical**:

- [`plans/architecture-review-full-codebase-v0.1.md`](architecture-review-full-codebase-v0.1.md) Finding 1 (save bypasses proposal) — **Historical**. Manual and generic save proposals now route through `SaveWorkflowService`/`AppComposition::apply_save_file_proposal()` and `WorkspaceActor::save_file_with_proposal()`. Registered open-buffer text edits, closed-file create/delete/rename proposals, multi-file workspace edits, edit-only code actions, and accepted reversible batches also execute through editor/workspace authorities; remaining gated areas are raw format execution and future runtime surfaces.
- [`plans/architecture-review-full-codebase-v0.1.md`](architecture-review-full-codebase-v0.1.md) Finding 3 (service ports not implemented) — **Partially historical**. `ProposalPort` is implemented by `AppProposalCoordinator`. `StorageRepositoryPort` has in-memory and file-backed implementations. `EditorPort` adapter remains a future refinement, not a Phase 0/1 blocker.
- [`plans/architecture-review-full-codebase-v0.1.md`](architecture-review-full-codebase-v0.1.md) mermaid diagram labeled "missing" for Proposal/Observability/Storage — **Historical**. Proposal lifecycle, observability event emission, storage audit persistence, workspace-authorized audit-failure rollback, and live ledger projection are now wired for accepted Phase 2 proposal routes. The remaining gap is universal mediation for batch/multi-file and later ADR-gated runtime classes.

---

## Dependency policy / `xtask` alignment decision

**Decision**: keep both.

- `plans/dependency-policy.md` remains the human-readable authority for directional intent, forbidden edges, shared contract boundaries, and runtime-surface activation gates.
- `xtask/src/main.rs` retains hardcoded checks for:
  - required protocol symbol presence in `crates/devil-protocol/src/lib.rs`
  - Phase 3 evidence artifact existence
  - Phase 3 / LSP acceptance-state marker validation
  - required internal dependencies that are easier to express in code than markdown parsing

Rationale: the markdown file is the source of truth for crate-level allowed-dependency sets, but literal string checks (e.g., `PHASE3_NOT_ACCEPTED_MARKER`) and symbol regexes are more maintainable as code. This is documented and accepted.

---

## Immediate implementation order

1. **R0 — Foundation lock** (this ledger and ADR reconciliation) — complete.
2. **R1 — Phase 2B generalized proposal execution** — complete and accepted.
3. **R2 — Phase 3A semantic-index boundary remediation** — complete and accepted.
4. **R3 — Phase 3B predictive semantic fabric and LSP supervision** — complete and accepted for Phase 3 scope.
5. **R4–R8** — blocked on individual ADR/policy gates, not on Phase 2 or Phase 3 acceptance.

---

## Exit criteria for this ledger

- [x] `cargo run -p xtask -- check-deps` passes.
- [x] Phase 0 and Phase 1 are explicitly accepted.
- [x] Phase 2 is explicitly accepted.
- [x] Phase 3 is explicitly accepted.
- [x] Phases 4–8 are explicitly future-gated.
- [x] ADR-0002 and ADR-0005 ambiguity is resolved.
- [x] Historical claims are annotated as historical.
- [x] Dependency-policy / `xtask` duplication decision is recorded.
