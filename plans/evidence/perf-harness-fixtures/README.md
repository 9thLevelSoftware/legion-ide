# Perf harness fixture benchmarks

> **Claim repair, 2026-08-16.** This file previously stated that these manifests
> "back the `xtask perf-harness` large-fixture search benchmarks" and are "read by
> the harness at runtime". Neither is true in the current tree. Verified by
> `rg -n "perf-harness-fixtures|large_fixture_search|fixture_file_count|search_scan_limit" xtask crates -g "*.rs"`,
> which returns no matches: no Rust source reads these files, and `SkeletonKind`
> in `xtask/src/perf_harness.rs` has no `large_fixture_search` variant to
> deserialize `kind` into. See
> `plans/evidence/production/PR-UI-001/2026-08-16-promotion-verification.md`.

**Status: inert.** These manifests are a design intent for `P2.F4.T4` /
`P8.F4.T1`. They are not loaded by any command and no measurement is produced
from them.

- `50k-file-search.toml`: intended bounded search over a 50K-file fixture corpus.
  Not wired. The harness's own `m8.search_stream_50k` skeleton is a separate,
  hardcoded descriptor in `xtask/src/perf_harness.rs` that generates its own
  50K-file fixture under the system temp directory at runtime and carries
  `budget_millis = 0`, so it is report-only and cannot fail a run.
- `100k-file-search.toml`: intended bounded search over a 100K-file fixture
  corpus. Not wired. The generation command that
  `plans/evidence/production/WS-MANUAL-02/reference-workspaces.md` cites for the
  100K-file reference workspace, `xtask generate-test-workspace`, does not exist
  as an `xtask` subcommand.

**Update, 2026-08-19 (P8.F4.T1).** A 100K-file workload now exists and runs in
every `xtask perf-harness` run — `p8.fixture_100k_files` in
`crates/legion-app/src/bin/product_perf.rs`, which generates its own fixture
under the system temp directory and searches it through the real product
`RunSearch` path. It does **not** read these manifests: the file count,
directory layout, and needle are constants in that binary. So these two files
are still inert, and still must not be cited as evidence that anything reads
them — but the sentence "there is no 100K-file workload in the harness at all"
is no longer true. See
`plans/evidence/production/P8.F4/perf-harness-product-workloads.md`.
