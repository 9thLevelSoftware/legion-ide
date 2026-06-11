# M0 — WS17.T1 (Release Pipeline) Bootstrap Evidence

Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Workstream: **WS-17** — Distribution, Updates & Crash Reporting
Plan task: **WS17.T1 Release pipeline** (master-plan v0.1 §7 line 427;
see `plans/legion-production-master-plan-v0.1.md`)
Date: 2026-06-10
Kanban card: `t_025ea8f0`

## Acceptance target

> "tagged commit yields all artifacts in CI; dry-run descriptors are verifiable."

The plan asks for a `cargo-dist`-based multi-platform CI (plan/build/host) producing
`dmg + msi (WiX) + deb/rpm/AppImage` with reproducible version stamping, a
release-channel model (stable/preview), and installer descriptors that record
name, platform, sha256, build command, verification command, and signer status
(`dry-run/no-production-signer` until real signing exists).

This card lands the M0 bootstrap: a deterministic, fail-closed dry-run scaffold
that owns the plan/build/host contract shape (descriptor TOML + version stamp +
verification report) and exercises it through the existing phase-gate tool
(`cargo run -p xtask -- …`). The actual `cargo dist build` invocation, artifact
upload, signing, and notarization are owned by **WS17.T2** and remain deferred
per the plan.

## What landed in this card

| Artifact | Purpose |
| --- | --- |
| `xtask/src/release_pipeline.rs` | Plan / write / verify core. Adds `VersionStamp`, `VerificationReport`, `channel_rollout_policy`, and reproducible SHA + RFC3339 build-time capture. |
| `xtask/src/main.rs` | New `release-pipeline` subcommand (stable / preview / `--dry-run`) and new `verify-release-pipeline` subcommand. |
| `xtask/tests/release_pipeline.rs` | 13 tests covering determinism, dry-run signer status, preview vs stable channel, idempotent writes, non-dry-run rejection, config load, rollout policy, version stamp fields, git SHA capture, descriptor round-trip, and verify report contents. |
| `xtask/release-pipeline.example.toml` | Six installer targets (macOS × 2, Windows × 1, Linux × 3) with build + verification commands. Header documents channel → rollout policy. |
| `.github/workflows/ci.yml` | Adds a tag-triggered `release` job (`refs/tags/v*.*.*`) that re-runs both channels on every OS matrix leg and asserts the verify pass. Existing `validate` job now also runs `verify-release-pipeline` after the descriptor dry run. `workflow_dispatch` enabled for ad-hoc runs. |
| `AGENTS.md` | New phase-gate line for `verify-release-pipeline` and a WS17.T1 note describing the descriptor / stamp / report contract and the dry-run posture (no private keys, no notarization creds, `dry-run/no-production-signer` only). |
| `plans/evidence/production/M0/WS17-T1-release-pipeline.md` | This file. |

## Plan-acceptance traceability

| Plan requirement | Implementation |
| --- | --- |
| `cargo-dist`-based multi-platform CI | `xtask/release-pipeline.example.toml` declares six `installer_targets` with `build_command = "cargo dist build --target <triple>"` and per-platform `verification_command`. CI re-runs them per OS matrix leg. |
| `dmg + msi (WiX) + deb/rpm/AppImage` | `legion-desktop-macos-{x64,arm64}-dmg`, `legion-desktop-windows-x64-msi`, `legion-desktop-linux-x64-{deb,rpm,appimage}` are all in the example config and produced as descriptors in `target/release-pipeline/`. |
| Reproducible version stamping | `VersionStamp` records `schema_version`, `package_name`, `package_version` (from `workspace.package.version`), `channel`, `rollout_policy`, `dist_tool`, `git_sha` (`git -C workspace rev-parse HEAD`, normalized to `"unknown"` if git is unavailable), and `built_at_utc` (RFC3339 via Howard Hinnant's `civil_from_days` — no `chrono` / `time` crate introduced). The stamp is written to its own `version_stamp.toml` and inlined into every descriptor. |
| Release-channel model (stable/preview) | `ReleaseChannel::parse` / `as_str` already existed; this card wires the channel label into the descriptor's `channel` field, derives `version = "<ws>-preview"` for preview, derives `rollout_policy` (`full` for stable, `staged` for preview) for WS17.T3 to consume, and exposes `--channel {stable,preview}` on the CLI. |
| Installer descriptors record name, platform, sha256, build command, verification command, signer status | `InstallerDescriptor` is unchanged in shape; the new `version_stamp` field is additive. `signer_status` stays `DRY_RUN_SIGNER_STATUS = "dry-run/no-production-signer"`. |
| `dry-run` semantics until real signing | `plan_release_pipeline` rejects `dry_run=false` with `"release pipeline currently supports descriptor dry-run only"`. Test `release_pipeline_rejects_non_dry_run_until_signing_policy_exists` enforces this. |
| "tagged commit yields all artifacts in CI" | The new `release` job in `.github/workflows/ci.yml` is gated on `startsWith(github.ref, 'refs/tags/v')` and runs the descriptor dry-run for both channels plus the verify pass on every matrix OS. |
| "dry-run descriptors are verifiable" | New `verify_descriptors` walks the on-disk descriptors, cross-checks each one against the plan, and writes `verify_report.toml` with per-descriptor `verifier_status` and a `summary { total, passed, failed, unchecked }`. `verify-release-pipeline` exits non-zero when any descriptor is missing (`failed/missing-descriptor`). |

## Commands run (with exit codes)

```
cargo run -p xtask -- check-deps                  → 0  ("dependency policy checks passed")
cargo run -p xtask -- docs-hygiene                 → 0  ("documentation hygiene checks passed")
cargo run -p xtask -- no-egui-textedit             → 0  ("no-egui-textedit checks passed")
cargo run -p xtask -- release-pipeline --dry-run   → 0  ("release pipeline dry-run wrote 7 descriptor(s) to target/release-pipeline")
cargo run -p xtask -- verify-release-pipeline      → 0  ("release pipeline verify: total=6 passed=0 failed=0 unchecked=6 channel=stable report=…/verify_report.toml")
cargo test -p xtask                                → 0  (13 release_pipeline + 6 no_egui_textedit = 19 passed; 0 failed)
cargo fmt -p xtask --check                         → 0
cargo clippy -p xtask --all-targets -- -D warnings → 0
cargo check --workspace --all-targets              → 0
```

A full `cargo test --workspace --all-targets` re-run was attempted but the
runner disk filled from sibling M0 cards' `target/` (100 % capacity, 117 MiB
free); the previous baseline of **1030 passed / 3 ignored** still stands from
the gate recorded on the parent task and the new xtask tests (19 pass) and CLI
gates (5/5) are confirmed green on the same machine.

## Generated artifacts (local dry run)

```
target/release-pipeline/version_stamp.toml
target/release-pipeline/verify_report.toml
target/release-pipeline/legion-desktop-macos-x64-dmg.toml
target/release-pipeline/legion-desktop-macos-arm64-dmg.toml
target/release-pipeline/legion-desktop-windows-x64-msi.toml
target/release-pipeline/legion-desktop-linux-x64-deb.toml
target/release-pipeline/legion-desktop-linux-x64-rpm.toml
target/release-pipeline/legion-desktop-linux-x64-appimage.toml
```

`version_stamp.toml` body (current HEAD `b56dcb2`):

```toml
schema_version = 1
package_name = "legion-desktop"
package_version = "0.1.0"
channel = "stable"
rollout_policy = "full"
dist_tool = "cargo-dist"
git_sha = "b56dcb20886f5ed582f7b7e004a7e5f93d8385b7"
built_at_utc = "2026-06-11T03:41:33Z"
```

`verify_report.toml` summary (current HEAD `b56dcb2`):

```toml
[summary]
total = 6
passed = 0
failed = 0
unchecked = 6
```

## Deferred (explicit cut line)

- Real `cargo dist build` artifact upload is owned by **WS17.T2** (Signing &
  notarization). Until that lands:
  - `sha256` stays `"pending"`.
  - `signer_status` stays `dry-run/no-production-signer`.
  - `verifier_status` stays `dry-run/unchecked` (real signing/verification is
    the body that WS17.T2 will replace inside `verify_descriptors`).
- Auto-update / rollback flow on top of the descriptors is **WS17.T3** and
  consumes `rollout_policy` from the version stamp.
- Crash reporting opt-in path is **WS17.T4** (separate workstream).

## Repository invariants

No changes to `legion-ui` / `legion-text` / `legion-editor` / `legion-app` /
`legion-protocol` / `legion-collaboration`. No new runtime dependencies were
added to the workspace `Cargo.toml`. `xtask` only gained the deterministic
git-SHA + RFC3339 helpers; the rest is `serde` + `toml` which were already
declared in `xtask/Cargo.toml`. No private keys, certs, tokens, or
notarization credentials were introduced.
