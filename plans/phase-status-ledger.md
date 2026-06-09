# Legion IDE Phase Status Ledger

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
| Phase 3 — Semantic fabric and LSP supervision | **Accepted** | [`plans/evidence/phase-3/predictive-semantic-fabric.md`](evidence/phase-3/predictive-semantic-fabric.md) | `legion-index` is activated for actor-owned bounded scheduling, workspace-authored discovery import, descriptor/lease indexing, syntax-cache freshness, graph records, metadata-only persistence, semantic query APIs, and LSP supervision/proposal-routing DTOs. Vector indexing and later runtime surfaces remain deferred. |
| Phase 4 — Native agentic AI execution context | **Accepted** | [`plans/evidence/phase-4/agentic-ai-architecture-map.md`](evidence/phase-4/agentic-ai-architecture-map.md) | Local-provider, proposal-only, metadata-audited agent execution is accepted. Cloud providers, hosted telemetry, hosted embeddings, gateways, vector storage/retrieval, terminal execution, collaboration, and remote development remain deferred. |
| Phase 5 — WASM isolated extension ecosystem | **Accepted** | [`plans/evidence/phase-5/plugin-architecture-map.md`](evidence/phase-5/plugin-architecture-map.md) | Manifest-validated, capability-checked, quota-bound, metadata-only plugin runtime boundary is accepted. Plugin command invocation is app-owned and protocol-mediated; UI remains projection-only. Marketplace, VS Code compatibility, Node extensions, arbitrary host scripting, network/process/filesystem/terminal authority, collaboration, and remote development remain deferred. |
| Phase 6 — Collaboration substrate | **Accepted** | [`plans/evidence/phase-6/collaboration-architecture-map.md`](evidence/phase-6/collaboration-architecture-map.md) | Local deterministic collaboration runtime, app-owned session composition/transport envelopes, shared proposal approval gates, reconnect/shutdown lifecycle semantics, metadata-only audit/replay, projection-only UI, and p95/p99 editor overhead evidence are accepted. Production remote network transport, standalone terminal/process execution, hosted telemetry, and raw-source retention remain deferred. |
| Phase 7 — Remote development | **Accepted** | [`plans/evidence/phase-7/remote-architecture-map.md`](evidence/phase-7/remote-architecture-map.md) | Deterministic edge workspace runtime harness, app-owned remote session composition, proposal-gated remote fixture filesystem mutation, bounded process/PTY/LSP/semantic descriptors, reconnect/offline metadata, security policy gates, and metadata-only audit/storage are accepted. Production network transport, standalone local terminal runtime, hosted telemetry, raw-source retention, and Phase 8 operational hardening remain deferred. |
| Phase 8 — Hardening | **Substrate accepted** | [`plans/evidence/phase-8/`](evidence/phase-8/) | Phase 8 runtime-hardening substrate acceptance: Accepted. The accepted substrate evidence includes `phase-8-architecture-map.md`, `platform-matrix-evidence.txt`, and `release-readiness-review.md`. This acceptance covers the runtime-hardening substrate (production remote transport, standalone local terminal, hosted telemetry/egress, raw-source retention, operational hardening) and is **not** a product GA or release-readiness sign-off. Product GA / release readiness is a separate, post-substrate track that requires additional GUI productization evidence (renderer-backed latency, IME, clipboard, focus, and accessibility), packaging/signing/auto-update decisions, and collaboration/admin/runtime-extension surface decisions; none of those reopen accepted Phase 8 substrate hardening. |

---

## ADR status reconciliation

| ADR | Status | Blocker or reservation |
| --- | --- | --- |
| ADR-0001 — Rust workspace | Accepted | — |
| ADR-0002 — UI/editor rendering | **Accepted with reservations** | Renderer-backed p50/p95 input-to-paint, IME, clipboard, focus, and accessibility evidence are follow-ups. Spike 1A is accepted with reservations in [`plans/spikes/SPIKE-001A-result.md`](spikes/SPIKE-001A-result.md). |
| ADR-0003 — Editor core text model | Accepted | Large-file and retained-history benchmark reservations recorded in Phase 1 evidence. |
| ADR-0004 — Async runtime actor model | Accepted | — |
| ADR-0005 — Storage backends | **Accepted with reservations** | Spike 3 vector-store evaluation is deferred. SQLite/Tantivy metadata baseline is accepted; durable semantic/tracker/memory/plugin/collaboration/replay storage requires follow-up ADR. |
| ADR-0006 — AI provider abstraction | Accepted for Phase 4 local-provider slice | Deterministic local/provider-router behavior is accepted through Phase 4 evidence. Cloud providers remain deferred behind provider-specific gates. |
| ADR-0007 — Mode policy engine | Accepted | — |
| ADR-0008 — Tracker schema | Accepted for Phase 4 metadata ledger | Tracker runtime is limited to metadata-only run ledger records and protocol/storage-mediated evidence. |
| ADR-0009 — Memory consent | Accepted for Phase 4 metadata retention | Memory runtime is limited to candidate review, explicit consent, deletion, and metadata-only retention. Vector retrieval remains deferred. |
| ADR-0010 — Air-gap mode | Accepted for Phase 4 provider policy | Air-gap and local-provider-only policy denies hosted providers, hosted telemetry, hosted embeddings, gateways, and unapproved outbound access. |
| ADR-0015 — Streaming text viewport | Accepted | — |
| ADR-0016 — Generalized proposal service | Accepted | Phase 2 accepted for save, text edit, closed-file, workspace-edit, edit-only code-action, and reversible batch proposal routes. Future runtime apply surfaces remain separately gated. |
| ADR-0017 — Semantic fabric indexing | Accepted | Phase 3 evidence accepts the bounded semantic fabric runtime; vector indexing remains deferred. |
| ADR-0018 — LSP runtime supervision | Accepted | Phase 3 evidence accepts metadata-only LSP supervision contracts and proposal-routed edit outputs; command/process/runtime expansion remains separately gated. |
| ADR-0019 — WASM plugin runtime boundary | Accepted | Phase 5 evidence accepts the isolated plugin boundary for manifest validation, host-call capability checks, quotas, plugin storage, metadata-only observability, app-owned command invocation, and projection-only UI contributions. |
| ADR-0020 — Collaboration operation model | Accepted | Phase 6 evidence accepts deterministic operation-log runtime behavior, editor-authority application, app-owned transport composition, replay metadata, and lifecycle fail-closed semantics. |
| ADR-0021 — Collaboration identity, permissions, and retention | Accepted | Phase 6 evidence accepts collaboration identities, capability-gated runtime activation, shared proposal approvals, metadata-only audit/replay storage, and projection-only UI presence. |
| ADR-0022 — Remote edge workspace agent | Accepted | Phase 7 evidence accepts a default-off deterministic edge workspace runtime harness with app-owned composition and proposal-mediated mutation boundaries. |
| ADR-0023 — Remote transport security | Accepted | Phase 7 evidence accepts metadata-only transport envelopes, trust/capability gating, and deferred production network hardening. |
| ADR-0024 — Remote execution boundary | Accepted | Phase 7 evidence accepts bounded descriptor-only process, PTY, LSP, and semantic-query surfaces without activating standalone local terminal or LSP runtimes. |
| ADR-0025 — Production remote network transport | Accepted | Phase 8 substrate evidence archives the production transport runtime, security, platform, fault, and ownership evidence. Substrate acceptance is not a product GA / release-readiness sign-off. |
| ADR-0026 — Standalone local terminal runtime | Accepted | Phase 8 substrate evidence archives native PTY runtime, policy, cleanup, platform, and privacy evidence. Substrate acceptance is not a product GA / release-readiness sign-off. |
| ADR-0027 — Hosted telemetry and egress | Accepted | Phase 8 substrate evidence archives durable spool, hosted exporter, consent, classifier, failure-mode, and operations evidence. Substrate acceptance is not a product GA / release-readiness sign-off. |
| ADR-0028 — Raw-source retention | Accepted | Phase 8 substrate evidence archives encrypted vault, consent, deletion, recovery, and privacy evidence. Substrate acceptance is not a product GA / release-readiness sign-off. |
| ADR-0029 — Phase 8 operational hardening | Accepted | Phase 8 substrate evidence archives migration/recovery, diagnostics, platform, performance, fault, cargo-deny, rollback, canary, incident, and final gate evidence. Substrate acceptance is not a product GA / release-readiness sign-off. |

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
  - required protocol symbol presence in `crates/legion-protocol/src/lib.rs`
  - Phase 3 evidence artifact existence
  - Phase 3 / LSP acceptance-state marker validation
  - Phase 8 evidence artifact names and accepted/not-accepted governance validation
  - required internal dependencies that are easier to express in code than markdown parsing

Rationale: the markdown file is the source of truth for crate-level allowed-dependency sets, but literal string checks (e.g., `PHASE3_NOT_ACCEPTED_MARKER`, `PHASE8_NOT_ACCEPTED_MARKER`) and symbol regexes are more maintainable as code. This is documented and accepted.

---

## Immediate implementation order

1. **R0 — Foundation lock** (this ledger and ADR reconciliation) — complete.
2. **R1 — Phase 2B generalized proposal execution** — complete and accepted.
3. **R2 — Phase 3A semantic-index boundary remediation** — complete and accepted.
4. **R3 — Phase 3B predictive semantic fabric and LSP supervision** — complete and accepted for Phase 3 scope.
5. **R4 — Phase 4 native agentic AI execution context** — complete and accepted for the local-provider, proposal-only, metadata-audited runtime slice.
6. **R5 — Phase 5 WASM isolated extension ecosystem** — complete and accepted for the manifest-validated, capability-checked, metadata-only plugin runtime boundary.
7. **R6 — Collaboration substrate** — complete and accepted for the local deterministic, app-owned, metadata-only collaboration substrate.
8. **R7 — Remote development** — complete and accepted for the deterministic edge workspace runtime harness and app-owned local projection scope.
9. **R8 — Hardening** — complete and accepted for the archived Phase 8 runtime-hardening substrate evidence. GUI productization starts after this substrate acceptance and does not reopen accepted Phase 8 runtime hardening; product GA / release readiness remains a separate, post-substrate track.

---

## Exit criteria for this ledger

- [x] `cargo run -p xtask -- check-deps` passes.
- [x] Phase 0 and Phase 1 are explicitly accepted.
- [x] Phase 2 is explicitly accepted.
- [x] Phase 3 is explicitly accepted.
- [x] Phase 4 is explicitly accepted for local-provider, proposal-only, metadata-audited agent execution.
- [x] Phase 5 is explicitly accepted.
- [x] Phase 6 is explicitly accepted.
- [x] Phase 7 is explicitly accepted for deterministic edge workspace runtime scope.
- [x] Phase 8 acceptance: Substrate accepted (runtime hardening). This is **not** a product GA / release-readiness sign-off; product GA / release readiness is a separate, post-substrate track.
- [x] GUI productization follow-up evidence is tracked as post-substrate work, not as a Phase 8 acceptance blocker.
- [x] ADR-0002 and ADR-0005 ambiguity is resolved.
- [x] Historical claims are annotated as historical.
- [x] Dependency-policy / `xtask` duplication decision is recorded.
