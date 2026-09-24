# S0-01n context, index, and memory scope audit

Date: 2026-09-05. Baseline: `828d1fd`. This is an inventory of committed source; product, native, configuration, and external acceptance remain `unassessed`.

The approved family-11 promise is `Context/retrieval`: index freshness, symbols, memory, provenance, budgets, privacy, and inspectable selection. S3-04 requires one app-owned assembler, freshness/provenance, bounded selection, memory controls, a remote boundary, inspection before egress, and cache behavior. WS-AI-02 adds manifest schema, preview, citations, rule discovery, and context-window budgeting. The remaining-tasks plan records accepted metadata-only semantic fabric and keeps vector/model retrieval gated.

| ID | Atomic outcome | Current trace and limit |
|---|---|---|
| COMP-CTX-001 | App-owned manifest includes files/buffers, symbols, diagnostics, terminal/Git context, memory, selections, route, budget, privacy, and egress identity. | `crates/legion-ai/src/manifest.rs` has manifest/source models and metadata fields; end-to-end app assembly is not evidenced. `partial`. |
| COMP-CTX-002 | Freshness-aware index/context selection covers identity, content, snapshot, generation, privacy, schema, parser/model, and cancellation. | `crates/legion-index/src/lib.rs` has bounded index/snapshot/lease machinery; complete app orchestration is unproven. `partial`. |
| COMP-CTX-003 | Retrieval returns symbols/relations/chunks with bounded ranking and provenance; vector/model retrieval stays qualified. | Index graph/query APIs and manifest provenance fields exist; no vector/model-backed retrieval is claimed. `partial`. |
| COMP-CTX-004 | Memory is opt-in, policy-scoped, identity/retention aware, reviewable/excludable, and cannot silently inject stale/private records. | `crates/legion-memory/src/lib.rs` has consent, validation, snapshots, compaction, and metadata summaries; app assembler integration is not evidenced. `partial`. |
| COMP-CTX-005 | Items/citations carry identity, fingerprint, freshness, reason, privacy, redaction, and reproducible provenance without raw retention by default. | Manifest, memory, and retention types carry metadata/privacy/redaction hints; complete citation/replay binding is unproven. `partial`. |
| COMP-CTX-006 | Deterministic byte/token/window budgets explain exclusions and bind reviewed fingerprint to dispatch, failing stale. | Manifest models and redaction bounding exist; reviewed-bundle preflight and provider equality/stale failure are not evidenced. `partial`. |
| COMP-CTX-007 | Privacy boundaries cover ignored/secret/generated/binary/oversized/symlink/remote/excluded sources with visible pre-egress redaction/denial. | `crates/legion-ai/src/redaction.rs`, `crates/legion-security/src/lib.rs`, and retention checks provide policy traces; complete source-selection enforcement is absent. `partial`. |
| COMP-CTX-008 | Projection-only inspector supports inspect/include/exclude, planned-vs-actual context, route/privacy/egress, budgets, citations, and cache state. | Existing assistant/projection surfaces are context only; dedicated manifest inspector interaction evidence is absent. `partial`. |
| COMP-CTX-009 | Metadata/fingerprint/citation replay works without raw retention and restart/rebuild preserves truthful freshness. | Memory snapshots and retention metadata support bounded inputs; integrated context replay and index restart evidence are absent. `partial`. |
| COMP-CTX-010 | Packaged native acceptance covers indexing, retrieval, memory, privacy, budgets, inspection, payload equality, restart, OS, and external oracles. | `xtask` EvidenceRun schema exists, but no required family-11 records were found. `absent`. |

The aggregate `COMP-SCOPE-FAMILY-11` is replaced by these ten rows. Existing legacy outcomes remain canonical and are mapped exactly below rather than duplicated. Models, fixtures, projections, and unit tests are implementation traces only. No row claims product/native/external acceptance. The requirements file has 316 rows (307 - 1 + 10); retained rows are field-equal after explicit UTF-8 decoding of the `828d1fd` Git blob, and JSON parses with unique IDs.

## Exact retained legacy mapping

| Retained legacy ID (title) | Exact overlap | Boundary preserved by new rows |
|---|---|---|
| `COMP-P4-F2-T1-1` — “Manifest is a structured DTO, not a freeform prompt.” | `COMP-CTX-001` manifest structure and `COMP-CTX-005` item metadata | CTX adds the complete app-owned source set, freshness, privacy, and provenance; it does not duplicate the DTO claim. |
| `COMP-P4-F2-T2-1` — “User can answer ‘what left my machine?’ before each AI run.” | `COMP-CTX-008` planned context/egress inspection and `COMP-CTX-007` pre-egress privacy state | CTX adds selection, exclusion, and policy-boundary behavior; the retained before-run disclosure remains canonical. |
| `COMP-P4-F2-T3-1` — “The bytes sent equal the manifest minus redacted items, with no other delta.” | `COMP-CTX-006` reviewed bundle/provider binding and `COMP-CTX-007` redaction | CTX adds deterministic budgets and source privacy cases; it does not restate the retained payload-equality oracle. |
| `COMP-P4-F2-T4-1` — “User can answer ‘what left my machine?’ after each AI run.” | `COMP-CTX-008` planned-versus-actual inspection and `COMP-CTX-009` replay metadata | CTX adds replay/recovery and citations; the retained after-run disclosure remains canonical. |
| `COMP-P3-F2-T3-1` — “Evidence panel shows structured fields, not freeform text dumps.” | `COMP-CTX-008` projection-only inspection and `COMP-CTX-009` replayable metadata | CTX adds context-specific controls and freshness; the retained evidence-panel formatting outcome remains canonical. |

These retained rows have source refs in `plans/kanban/legion-ga-backlog.toml`, `docs/superpowers/plans/2026-09-04-completion-traceability.md`, and `plans/evidence/production/M9/` in `requirements.json`; the new CTX rows intentionally keep empty `legacy_ids` because the retained outcomes remain the canonical owners.
