# Phase 6: Integration Testing & Release -- Context

## Phase Goal
End-to-end testing and release artifacts. Install from release artifact on clean machine, open project, edit, run, commit.

## Requirements Covered
- req_phase-6-integration-testing-release: Phase 6 (Integration Testing & Release) has a resolved planning source.

## What Already Exists (from prior phases)

### Test Infrastructure (2,155 tests across 28 crates)
- `crates/legion-app/tests/` — 45 integration test files covering LSP, git, terminal, search, delegated tasks, settings, palette, auto-updater
- `crates/legion-desktop/tests/` — 55 integration test files covering GUI workflows, keyboard nav, rendering, accessibility, platform integration
- `crates/legion-app/src/bin/golden_path_{1..4}.rs` — 4 golden-path E2E smoke binaries
- GP-1: Full user journey (open workspace → LSP → diagnostics → search → terminal → git commit → evidence report)
- GP-2-4: Additional workflows (details in respective files)

### CI/CD (4 GitHub Actions workflows)
- `.github/workflows/legion-gates.yml` — PR merge blocker: 3-OS matrix, xtask checks, cargo fmt/check/test/clippy, release-pipeline dry-run
- `.github/workflows/legion-smoke.yml` — Weekly GP-1-4 golden-path smokes + update-drill, 3-OS matrix
- `.github/workflows/legion-bench.yml` — Weekly recorded-mode bench suite, 3-OS matrix
- `.github/workflows/legion-preview.yml` — Weekly unsigned-beta preview bundles, 3-OS matrix

### Release Pipeline
- `xtask/src/release_pipeline.rs` — Full release pipeline: plan, write descriptors, verify, SHA-256 checksums. Currently runs dry-run only in CI.
- `xtask/src/signing.rs` — Ed25519 signing (DalekSigner, keyring/env/KMS resolution)
- `crates/legion-app/src/updater.rs` — Auto-updater client: Ed25519 verification, channel-locked, downgrade prevention. HTTP manifest source deferred.
- `crates/legion-protocol/src/release_manifest.rs` — ReleaseManifestV1 type with TOML serialization
- `scripts/package-windows.ps1`, `scripts/package-preview.{ps1,sh}` — Packaging scripts for all platforms
- `xtask/release-pipeline.example.toml` — Config defining 6 installer targets (macOS x64/ARM64, Windows x64, Linux DEB/RPM/AppImage)

### Platform Support
- 3-OS matrix in CI (ubuntu, windows, macos)
- `legion-platform` crate: PTY/ConPTY, process spawn, filesystem abstraction
- `legion-sandbox` crate: per-OS sandboxing (Landlock, sandbox-exec, Job Objects)
- Cross-platform GUI via eframe/egui

### Desktop Rendering (Phase 5 outputs)
- `crates/legion-desktop/src/theme.rs` — Theme system with DiagnosticTokens, SearchTokens, ChromeTokens, BackgroundTokens, BorderTokens, TextTokens, AccentTokens
- `crates/legion-desktop/src/view.rs` — Tab strip with close buttons + drag-to-reorder, code minimap, syntax highlight rendering, LSP diagnostic underlines, completion popup, hover tooltip

## Key Design Decisions
- GP-1 already proves the app-layer user journey works. Phase 6 focuses on desktop rendering verification and real artifact production.
- Dog-food testing is structured as "run full test suite + fix blockers" rather than open-ended manual testing.
- Release pipeline promotion is incremental — activate real builds without rewriting the existing dry-run infrastructure.
- Phase 6 does NOT implement: HTTP manifest source for auto-updater, remote development server, plugin marketplace, VS Code extension runtime, collaboration transport, hosted telemetry backend (all explicitly deferred in FINISH-PLAN.md).

## Plan Structure
- **Plan 06-01 (Wave 1)**: End-to-End User Journey Test Suite -- Desktop rendering integration tests proving syntax colors, LSP diagnostics, terminal, and git workflow render correctly
- **Plan 06-02 (Wave 1)**: Release Build & Package Verification -- Promote release pipeline from dry-run, add installation smoke test
- **Plan 06-03 (Wave 2)**: Acceptance Proof & Dog-Food Fixes -- Phase 6 evidence gate, acceptance test, any blocker fixes
