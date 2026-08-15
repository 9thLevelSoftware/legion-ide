# Phase 0.2 truth-and-baseline repair — closure evidence (P0.F4)

Date: 2026-08-15. Branch: `phase-0-truth-repair` (base `8cc415b`, tree-identical
to `origin/main` squash commit `86b2f7e`). Executed under
`plans/legion-production-roadmap-v1.0.md` Phase 0.

## P0.F4.T1 — dead-reference repair

Every current-doc reference to files deleted in the 2026-08-12 cleanup
(`ENGINEERING_AUDIT.yaml`, `ENGINEERING_STATUS.md`, `ENGINEERING_PLAN.yaml`,
`audit-reports/`, `.hermes/`, `docs/releases/v8.0.0/`) was repaired or
annotated as historical:

- `plans/product-readiness-ledger.md` — four rows (PR-AI-002, PR-VSC-002,
  PR-ENT-001, PR-ENT-002) re-anchored to ADR-0046/`AGENTS.md` with explicit
  removed-in-cleanup notes; a consistency note added to the Beta Acceptance
  Scenario (its VSIX/collaboration steps depend on deferred gates).
- `docs/INDEX.md` — removed/annotated ENGINEERING_STATUS, audit-reports,
  `.hermes` generator-plan, and v8.0.0 template entries; WS-A-D charter marked
  closed (2026-07-22).
- `README.md` — GUI-evidence and repository-hygiene sections repointed.
- `CONTRIBUTING.md` — historical-audit line replaced.
- `CODEBASE.md` — config table rows for the deleted YAML files removed;
  `legion-release.yml` trigger corrected to manual-dispatch-only with the
  `mode` input; completion-status table given an explicit scope caveat
  (finish-phase campaign ≠ readiness); feature-flag note updated.
- `docs/hygiene-allowlist.toml` — stale `audit-reports/` and
  `docs/releases/v8.0.0/` entries removed.

## P0.F4.T2 — backlog reconcile vs commit `5c09a24`

- `[meta].plan` repointed from the deleted `.hermes` generator plan to
  `plans/legion-production-master-plan-v0.2.md`.
- `[meta].milestone` `M12` → `M8` (earliest milestone with open tasks; the
  prior value overstated progress).
- `P1.F4.T2` `todo` → `in-progress` (streaming open path + scale tests landed
  in `5c09a24`; renderer-level acceptance still unproven). `P1.F4.T3/T4/T5`
  verified genuinely still `todo` (no distinct streaming projection state or
  UI badge exists in `legion-ui`).
- `P9.F3.T2` annotated ADR-0046-frozen (`in-progress` predated the freeze;
  validator reserves `blocked` for `EXT-*` externals, so the freeze is
  encoded as `todo` + note).
- `P8.F3.T2` audited: its `done` status already carries the
  native-minidump-deferred caveat inline — consistent with the ledger, no
  change required.

## P0.F4.T3 — `--no-default-features` build repair

`cargo check -p legion-app --no-default-features` failed with 13 errors on
`main`. Fix: `ProductChatCompletion` and `product_stream_from_completion` are
pure data/formatting — their `ai` gates removed so the stream-sink machinery
keeps one signature across configurations; `legion-sandbox` made a
non-optional dependency (sandboxed DAP adapter spawn is debugger security,
not AI). The configuration is now a CI step in `legion-gates.yml`.
**This same defect was the root cause of the hosted update-drill failures on
all three OSes** (the drill builds `upd-drill` with `--no-default-features`)
— see `2026-08-15-hosted-smoke-first-run.md`. Local
`cargo run -p xtask -- update-drill` passes with the fix.

## P0.F4.T4 / T6, P0.F5 — companion closures

- RCA leftovers (verifier script extraction + tests 11/11, Debian
  `Maintainer`, `VALIDATION-SUMMARY.toml` publish gate): see the checked-off
  plan at `docs/superpowers/plans/2026-08-12-native-release-e2e-rca-and-resolution.md`.
- GP-1 s3 rust-analyzer 1.97.1 pull-diagnostics fix: see
  `2026-08-15-hosted-smoke-first-run.md` (local GP-1 green, s3 6.3s).
- SmallCode governance: `plans/adrs/ADR-0049-smallcode-behavioral-cannibalization.md`,
  `THIRD_PARTY_NOTICES.md`, `docs/legal/smallcode-attribution.md`, and 76
  attributed test vectors under
  `crates/legion-ai/tests/fixtures/smallcode_vectors/`.

## Gate evidence

On this branch during the closure session (Windows, single-OS):
`docs-hygiene`, `claim-audit`, `verify-kanban-backlog`,
`verify-readiness-consistency`, `check-deps`, `cargo fmt --all --check`,
`cargo check -p legion-app` (with and without default features),
`cargo test --workspace --all-targets` (full suite, green),
`cargo clippy -p legion-app -p xtask --all-targets -- -D warnings`,
`cargo run -p xtask -- update-drill`, `cargo run -p xtask -- golden-path-1`
— all passing at commit time (final run recorded in the closing commit
message). Hosted 3-OS re-validation is the P0.F4.T5 promotion clock.
