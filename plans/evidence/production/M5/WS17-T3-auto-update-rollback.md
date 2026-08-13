# M5 — WS17.T3 Auto-Update + Rollback Evidence

## Status

Accepted for the current repository-local WS17.T3 scaffold: the release pipeline dry-run/verification surface is stable and the remaining production update → rollback e2e work is explicitly recorded below as external release-ops evidence, not claimed as complete product evidence.

## Acceptance target

- Update → rollback e2e on all 3 OSes.
- Staged rollout policy must remain encoded in the release metadata.
- Rollback criteria and incident response need to be explicit and reproducible.

## What was verified

- `xtask/src/release_pipeline.rs`
  - `VersionStamp.rollout_policy` remains derived from the release channel (`full` for stable, `staged` for preview).
  - `verify_descriptors()` now reads the written `version_stamp.toml` instead of regenerating a fresh timestamped plan, so verify stays stable even if the dry-run ages between write and verify.
  - Descriptor verification still fails closed on missing files and tampered bytes.
- `xtask/tests/release_pipeline.rs`
  - Added a regression test proving verification uses the written version stamp rather than a freshly generated timestamp.
  - The tamper-detection regression still rejects modified descriptor bytes.
- CLI verification on the current workspace
  - `cargo run -p xtask -- release-pipeline --dry-run` writes the descriptor set.
  - `cargo run -p xtask -- verify-release-pipeline` now passes after the dry-run, instead of failing on stale timestamp drift.
- Operational evidence already in the repo
  - `plans/evidence/gui-productization/phase-8-update-rollback-incident.md` documents rollback and incident-drill criteria.
  - `plans/evidence/gui-productization/phase-8-ga-release-runbook.md` documents the rollback/restore sequence and the metadata-only incident record shape.
  - `plans/evidence/production/M0/WS17-T1-release-pipeline.md` records that WS17.T3 consumes the rollout policy from the version stamp.

## Verification commands

```bash
cargo test -p xtask --test release_pipeline -- --nocapture
cargo run -p xtask -- release-pipeline --dry-run
cargo run -p xtask -- verify-release-pipeline
```

## Results

- `cargo test -p xtask --test release_pipeline -- --nocapture`
  - 15 tests passed, including the new stale-version-stamp regression.
- `cargo run -p xtask -- release-pipeline --dry-run`
  - Passed; wrote 7 descriptors to `target/release-pipeline/`.
- `cargo run -p xtask -- verify-release-pipeline`
  - Passed; `total=6 passed=0 failed=0 unchecked=6 channel=stable`.

## Blockers / deferred production evidence

These items remain required before PR-REL-001 can be represented as production-ready in the readiness ledger, but they are outside the repository-local dry-run scaffold that this card can verify without signing credentials and fresh OS VMs:

- No production update/install/rollback run has been performed on macOS, Windows, or Linux.
- No signed installers or fresh-VM verification evidence exist yet.
- The current release pipeline remains a dry-run scaffold rather than a shipped auto-update mechanism.

## Findings

- The timestamped version stamp made `verify-release-pipeline` unstable across time until verification started using the written stamp from disk.
- The repo now has a reliable dry-run verification path, but that is still not the same as update → rollback e2e coverage on all three OSes.
