# S0-01m Provider/model setup scope audit

This inventory replaces only `COMP-SCOPE-FAMILY-10` with `COMP-PROV-001..011`. The governing family is **“Provider/model setup”**: local and hosted configuration, credentials, managed local runtime, model download/integrity, hardware fit, and useful first-run setup (`docs/superpowers/specs/2026-09-04-product-completion-design.md`, Required feature families, family-10). All acceptance remains `unassessed`.

## Source and retained-row mapping

The full-vision specification, Stage 3 AI implementation plan, master/roadmap provider commitments, ADR-0006 credential boundary, and current provider/policy source were inspected. Existing P4.F1 rows are narrower retained outcomes and are mapped explicitly; they do not close the new managed-runtime or native-evidence scope.

| inspected source / retained row | mapping | current-source boundary |
|---|---|---|
| family-10 specification; S3-02 provider setup and S3-07 qualification | `PROV-001`, `PROV-011` | setup/qualification scope is required; current native provider journey is not accepted |
| `COMP-P4-F1-T1-1` hosted provider call requires explicit workspace consent | `PROV-005` | retained policy test is exact narrow overlap; full route/setup/evidence remains open |
| `COMP-P4-F1-T2-1` provider keys never enter tracked config | `PROV-002` | key-entry/redaction substrate exists; complete add/replace/revoke/delete native lifecycle remains unassessed |
| `COMP-P4-F1-T3-1` provider capabilities queryable and activation-gated | `PROV-003`, `PROV-004` | capability/policy substrate exists; complete matrix and user-visible health/feature coverage remains unassessed |
| `COMP-P4-F1-T4-1` recorded offline smoke/live smoke credential gate | `PROV-011` | retained smoke gate is supporting evidence only; it is not native first-run/provider qualification |
| `docs/superpowers/plans/2026-09-04-ai-team-completion.md`, S3-02 | `PROV-001..006` | defines setup, credentials, health, estimates, policy, and no-fallback outcomes |
| same plan, S3-03; production roadmap managed-runtime phase | `PROV-007..010` | defines runtime, catalog/download/integrity, hardware fit, and lifecycle recovery; committed tree has no complete managed implementation |
| `plans/adrs/ADR-0006-ai-provider-abstraction.md`, provider/platform/security/AI source | `PROV-002..006` | authority and metadata traces are implementation evidence, not native acceptance |

## Atomic outcomes and current classification

| ID | atomic outcome | implementation | evidence limit |
|---|---|---|---|
| PROV-001 | complete local/self-hosted/hosted provider and model setup/selection through useful first-run/settings flow | partial | setup rows and provider registry exist; complete matrix/native setup is unassessed |
| PROV-002 | secure credential add/replace/revoke/delete lifecycle with redacted projections and OS-backed references | partial | key-entry and platform/security seams exist; complete lifecycle/native proof is unassessed |
| PROV-003 | endpoint/model validation plus named provider health, latency, locality, and availability state | partial | provider setup/capability substrate exists; complete health panel is unassessed |
| PROV-004 | per-provider/model capability declarations and activation gates for streaming, tools, structured output, vision, embeddings, context, thinking, and cancellation | partial | capability DTOs and policy traces exist; full declared matrix is unassessed |
| PROV-005 | explicit consent, route/egress/retention/budget policy, Manual/air-gap denial, and no silent fallback | partial | policy and consent traces exist; native no-egress/provider route proof is unassessed |
| PROV-006 | preflight token/cost estimate, ceiling enforcement, and actual usage/cache/cost/latency reconciliation | partial | usage/observability models exist; complete setup-to-provider reconciliation is unassessed |
| PROV-007 | managed local runtime discovery/install and explicit external-runtime mode with bounded process supervision | partial | Ollama/llama.cpp adapters and diagnosis exist; managed supervisor is not present |
| PROV-008 | approved model catalog/import/download with consent, resumability, digest/provenance verification, and mismatch quarantine | absent | no committed managed catalog/downloader/import authority establishes this outcome |
| PROV-009 | disk/memory/accelerator/architecture/platform fit checks before acquisition/launch | absent | no committed provider hardware-fit implementation or native oracle establishes this outcome |
| PROV-010 | managed runtime/model cancellation, crash/backoff, restart, reuse, upgrade, uninstall, port collision, and orphan cleanup | absent | no committed managed lifecycle supervisor establishes this outcome |
| PROV-011 | packaged native first-run/provider acceptance across local/hosted routes, credentials, policy, capabilities, integrity, hardware, lifecycle, OS, and external oracles | absent | EvidenceRun schema exists, but required provider native records are not asserted present |

No acceptance is promoted. Existing retained rows are mapped only where their titles are semantically exact; deferred managed-runtime, model-acquisition, hardware-fit, lifecycle, and native qualification scope remains explicit.
