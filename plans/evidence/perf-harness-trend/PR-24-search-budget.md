# PR-24 Search Budget Evidence

## Decision

PR-24 does not replace the report-only 50K-file search budget. No qualifying
renderer-backed measurement is committed, so no measured ceiling is promoted.

## Evidence reviewed

- `m8.search_stream_50k` generates a temporary fixture and keeps
  `budget_millis = 0`; it is a report-only workload, not a renderer-backed
  product measurement.
- The fixture manifests under `plans/evidence/perf-harness-fixtures/` are inert
  documentation and are not loaded by the harness.
- The existing 100K-file trend note records approximately 74 seconds with a
  warm page cache and approximately 950 seconds with cold-cache real-time
  antivirus. Those runs do not provide a stable, renderer-backed 50K budget
  with reproducible provenance.

## Promotion status

The 50K search row remains report-only and untracked. This is intentional: a
budget derived from a synthetic fixture, a non-renderer path, or a single
machine-sensitive run would create a false promotion signal. A future PR may
replace this note only after recording a renderer-backed workload, machine/OS
profile, sample set, command provenance, and a repeatable measured ceiling.
