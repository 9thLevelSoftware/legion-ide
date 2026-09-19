# Production Qualification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Qualify Legion as a signed, accessible, privacy-preserving, production IDE through packaged native workflows, recovery drills, and independent release evidence.

**Architecture:** `xtask` remains credential-free policy/report verification; measurements live in product crates. Platform, performance, privacy, packaging, and procurement implementation starts as soon as its Stage 0 or Stage 1 contract exists, while the final production decision waits for accepted Stages 1–5. Stable artifacts are signed only in a protected manual release environment after owner-provided prerequisites are verified, and all product evidence uses the Stage 0 `EvidenceRun` JSON schema.

**Tech Stack:** Rust, eframe/egui/AccessKit, Windows UIA/Narrator, macOS AXUIElement/VoiceOver, Linux AT-SPI/Orca, GitHub Actions, canonical JSON evidence with hashed native/TOML attachments.

**Spec:** `docs/superpowers/specs/2026-09-04-product-completion-design.md`

## Global Constraints

- S0-01 through S0-05 own the normalized inventory, compatibility matrix, traces, acceptance fixtures, and evidence schema. Every `XQ-*` and `S6-*` record maps to their published `COMP-*` IDs.
- Component, projection, direct-action, headless, or replayed-model tests are supporting evidence only. A real-input packaged unsigned build may qualify a normal workflow at layer 3, but it cannot close signed-install, platform-trust, clean-machine, or stable-release acceptance.
- Preserve projection-only UI, proposal-mediated writes, default-deny egress, metadata-only observability, and fail-closed saves.
- Pull-request gates remain credential-free. Never commit or log signing keys, notarization credentials, provider keys, crash-upload secrets, or customer data.
- Existing procurement is a lead, not authority: Apple membership and Mac mini availability were owner-confirmed, while certificates/notarization API credentials remain unissued. Anthropic was ruled out on cost. Verify prerequisites from the current owner-maintained procurement record before release action, then request owner action only for a genuinely absent prerequisite; do not force a vendor or purchase.
- A final release has no incomplete required `COMP-*`, `XQ-*`, or `S6-*` record regardless of severity. Previews may retain named gaps but cannot be called production complete.
- `plans/completion/candidate.json` pins `code_sha` and artifact hashes. `EvidenceRun.candidate_sha` always means that pinned code SHA, even when evidence is committed later. Keep code, candidate build/hash, capture, and evidence commits separate; never rebuild or silently advance the candidate while collecting evidence.
- Historical captures under `plans/evidence/` are read-only inputs. New qualification results are written only to `plans/evidence/completion/<run-id>/` as canonical `EvidenceRun` records and hashed attachments. Do not modify or recommit historical captures as current results. Stage only exact changed documentation files and new run files, then inspect the staged diff.

---

## File map

| Files | Responsibility |
| --- | --- |
| `xtask/src/readiness_consistency.rs`, `xtask/tests/qual11_release_blocker.rs`, `plans/product-readiness-ledger.md` | Candidate evidence and fail-closed release decision. |
| S1-owned `crates/legion-desktop/src/native_acceptance.rs`, `xtask/src/completion_evidence.rs`, `xtask/tests/completion_evidence.rs` | Ingest packaged OS-input/accessibility results into canonical `EvidenceRun` JSON; preserve the existing direct-action windowed GUI smoke unchanged. |
| `scripts/a11y-uia-walk.ps1`, `scripts/a11y-ax-walk.sh`, `scripts/a11y-atspi-walk.sh`, `scripts/a11y-narrator-transcript.ps1`, `.github/workflows/legion-a11y-os-tree.yml` | UIA/AX/AT-SPI and real screen-reader evidence. |
| `xtask/src/perf_workloads.rs`, `xtask/src/perf_harness.rs`, `xtask/tests/perf_workloads.rs` | Calibrated renderer/product workload policy; `plans/evidence/perf-harness-trend/baseline.toml` is a read-only historical input. |
| `crates/legion-app/src/updater.rs`, new `crates/legion-app/src/bin/install_handoff.rs`, `crates/legion-app/Cargo.toml`, `crates/legion-app/src/bin/update_drill.rs`, `xtask/src/update_drill.rs` | Signed external helper, installed replacement/restart/rollback, and package ownership. |
| `crates/legion-observability/src/crash_capture.rs`, `crates/legion-app/src/diagnostics.rs`, `crates/legion-retention/tests/privacy_deletion.rs` | Crash recovery, diagnostics, privacy/deletion. |
| `xtask/src/release_pipeline.rs`, `xtask/release-pipeline.example.toml`, `.github/workflows/legion-release.yml`, `scripts/package-native.ps1`, `scripts/package-native.sh` | Native installers, signed stable descriptors, Manual artifact. |
| `plans/release/procurement-and-key-escrow.md`, `docs/OPERATOR_RUNBOOK.md`, `docs/PRIVACY.md`, `docs/SECURITY.md` | Owner actions, operator qualification and public claims. |

## Stage 6 boundary

`XQ-01` through `XQ-08` are cross-cutting implementation packages, not Stage 6 work. Each starts at its declared implementation stage once its Stage 0/Stage 1 contracts exist; procurement and platform preparation begin in Stage 0. The seven sequential milestones remain Stages 0–6, and Stage 6 contains only final candidate qualification. `S6-01` starts after accepted Stages 1–5 and accepted `XQ-01..08` evidence. Its candidate record names SHA, installer hashes, OS/tool/accessibility matrix cells, all `COMP-*` rows, external prerequisites, evidence, defects, and reviewers. The master completion plan owns the canonical requirement, matrix, scenario, candidate, and evidence schemas; this plan adds qualification-specific rows without redefining them. Every `verify-completion` invocation reads `code_sha` from `plans/completion/candidate.json`; the final decision also supplies `--release`.

### Task XQ-01: Add production qualification rows to the canonical completion register

**Implementation stage:** Stage 0.

**Files:**
- Create: `plans/evidence/completion/<run-id>/README.md`
- Modify: `xtask/src/completion.rs` (created by S0-03)
- Modify: `xtask/tests/completion.rs` (created by S0-03)
- Modify: `xtask/Cargo.toml`, `Cargo.lock`
- Create: `xtask/tests/support/completion_fixture.rs`
- Modify: `plans/completion/requirements.json` (created by S0-01)
- Modify: `plans/completion/matrix.json` and `plans/completion/scenarios.json` (created by S0-02)

**Depends on:** S0-01, S0-02, S0-03, S0-04, S0-05.

**Produces:** canonical requirement/scenario/matrix rows for `XQ-01..XQ-08` and final `S6-01`, consumed by `validate_completion(root, candidate, release)` and `xtask verify-completion`; add `tempfile = { workspace = true }` under `xtask` dev-dependencies. Test helper `write_valid_development_fixture(root: &Path, candidate: &str) -> Result<(), String>` writes a complete valid Stage 0 fixture which each negative test mutates once. XQ-01 also adds `validate_dogfood_campaign(root: &Path, candidate: &str) -> Result<Vec<String>, String>` and its missing-recovery fixture test so Stage 6 performs qualification only.

- [ ] **Step 1: Write the failing validator test**

```rust
#[test]
fn production_completion_rejects_required_qualification_without_evidence() {
    let root = tempfile::tempdir().expect("temporary completion fixture");
    let candidate = "0123456789abcdef0123456789abcdef01234567";
    write_valid_development_fixture(root.path(), candidate).expect("valid fixture");
    remove_evidence_for(root.path(), "XQ-03").expect("remove one fixture artifact");
    let violations = validate_completion(root.path(), candidate, true)
        .expect("fixture read");
    assert!(violations.iter().any(|v| v.contains("XQ-03") && v.contains("evidence")));
}
```

- [ ] **Step 2: Run:** `cargo test -p xtask --test completion production_completion_rejects_required_qualification_without_evidence`

Expected: if S0-03/04 already enforce the row, PASS and inspect the violation to confirm it is evidence-based; otherwise FAIL for the missing XQ-03 evidence rule, then implement only that missing rule.

- [ ] **Step 3: Implement fail-closed schema**

Add rows using the master schema: requirement `acceptance` remains `unassessed | blocked | failed | accepted`, while evidence `result` remains `passed | failed | blocked | skipped`. Each row names its S0 trace, required matrix cells, normal UI scenario, recovery scenario, evidence paths bound to `candidate.json.code_sha`, and independent reviewer where required. Reject missing S0 trace, evidence for a different pinned candidate SHA, empty independent reviewer, any required row not accepted from passed evidence, and a production-complete decision with unresolved requirements. Preview stays explicit/non-promotional. Define `remove_evidence_for(root: &Path, requirement_id: &str) -> Result<(), String>` beside the fixture builder so the shown test compiles. Implement `validate_dogfood_campaign` by requiring ten distinct owner working dates, two distinct independent reviewer identities, the nominated candidate on every run, and every scenario-defined recovery result; its test mutates a valid `EvidenceRun` to `result: "passed"` with an empty `recovery_results` array and expects rejection.

- [ ] **Step 4: Run:** `$candidateSha = (Get-Content -Raw plans/completion/candidate.json | ConvertFrom-Json).code_sha; cargo test -p xtask --test completion; cargo run -p xtask -- verify-completion --candidate $candidateSha; cargo run -p xtask -- verify-readiness-consistency`

Expected: PASS; malformed records fail with the owning ID.

- [ ] **Step 5: Commit:** stage the exact changed implementation/schema files and new `plans/evidence/completion/<run-id>/` files only; inspect the staged diff, then commit with a focused message.

### Task XQ-02: Ingest S1 native input and accessibility evidence on Windows/macOS/Linux

**Implementation stage:** Stage 1; language-specific evidence is added during Stage 2.

**Files:**
- Modify: `xtask/src/completion_evidence.rs` (created by S0-04)
- Modify: `xtask/tests/completion_evidence.rs` (created by S0-04)
- Create: `xtask/tests/support/evidence_fixture.rs`
- Modify: `scripts/a11y-uia-walk.ps1`, `scripts/a11y-ax-walk.sh`, `scripts/a11y-atspi-walk.sh`, `scripts/a11y-narrator-transcript.ps1`
- Modify: `.github/workflows/legion-a11y-os-tree.yml`, `.github/workflows/legion-windowed-gui.yml`
- Test: `crates/legion-desktop/tests/accessibility.rs`

**Depends on:** XQ-01 and S1-02 native driver implementation. Acceptance of each row depends on its corresponding Stage 1 Manual or Stage 2 language workflow.

**Produces:** canonical `EvidenceRun` JSON from the S1 command `xtask native-product-acceptance --scenario <id> --report <path>`, with UIA/AX/AT-SPI and screen-reader files listed in `artifact_hashes` and their scenario-defined checks in `oracle_results`. Test helper `write_valid_evidence_fixture() -> tempfile::TempDir` and mutation helper `mutate_evidence_run(root: &Path, edit: impl FnOnce(&mut serde_json::Value))` are defined in `xtask/tests/support/evidence_fixture.rs`. No second TOML report or native-acceptance type is introduced.

- [ ] **Step 1: Add a failing S0-04 evidence fixture test**

```rust
#[test]
fn native_acceptance_rejects_direct_dispatch_and_missing_accessibility_oracles() {
    let root = write_valid_evidence_fixture();
    mutate_evidence_run(root.path(), |run| {
        run["input_route"] = "runtime-dispatch".into();
        run["oracle_results"] = serde_json::json!([]);
    });
    let violations = validate_evidence_files(root.path(), CANDIDATE).unwrap();
    assert!(violations.iter().any(|v| v.contains("native-input")));
    assert!(violations.iter().any(|v| v.contains("accessibility-tree")));
}
```

- [ ] **Step 2: Run:** `cargo test -p xtask --test completion_evidence native_acceptance_rejects_direct_dispatch_and_missing_accessibility_oracles; cargo test -p legion-desktop --test accessibility; cargo test -p xtask --test windowed_gui_ci_contract`

Expected: FAIL until reports reject direct action dispatch.

- [ ] **Step 3: Implement and consume the S1 packaged OS-input acceptance route**

Extend the S1 scenario definitions with required oracle IDs `disk-edit`, `git-status`, `terminal-exit`, `language-diagnostic`, `accessibility-tree`, and `screen-reader`. The platform scripts emit attachments only; the S1 driver hashes them and writes a canonical `EvidenceRun`. Require Windows UIA + Narrator, macOS AX + VoiceOver, Linux AT-SPI + Orca. Missing permission, binding, process, tree, or assistive-technology session yields evidence `result: "blocked"` and requirement `acceptance: "blocked"`, never `passed`/`accepted`.

- [ ] **Step 4: Run each platform through `cargo run -p xtask -- native-product-acceptance --scenario manual-open-type-save --report plans/evidence/completion/native-manual-open-type-save/evidence.json`, then run `$candidateSha = (Get-Content -Raw plans/completion/candidate.json | ConvertFrom-Json).code_sha; cargo run -p xtask -- verify-completion --candidate $candidateSha`**

Expected: PASS only with package hash, OS driver, nonempty OS tree, AT outcome, and externally checked effects. Existing direct-action smoke remains smoke-only.

- [ ] **Step 5: Commit:** `git add xtask/src/completion_evidence.rs xtask/tests/completion_evidence.rs xtask/tests/support/evidence_fixture.rs crates/legion-desktop/tests/accessibility.rs scripts/a11y-uia-walk.ps1 scripts/a11y-ax-walk.sh scripts/a11y-atspi-walk.sh scripts/a11y-narrator-transcript.ps1 .github/workflows/legion-a11y-os-tree.yml .github/workflows/legion-windowed-gui.yml; git commit -m "test: require native accessible packaged workflows"`

### Task XQ-03: Calibrated product-performance qualification

**Implementation stage:** Stage 0 freezes calibration policy and reference hardware; Stage 1 adds renderer workloads; Stage 2–5 add their workloads as those workflows land.

**Files:**
- Modify: `xtask/src/perf_workloads.rs`, `xtask/src/perf_harness.rs`, `xtask/tests/perf_workloads.rs`, `.github/workflows/legion-gates.yml`
- Read-only input: `plans/evidence/perf-harness-trend/baseline.toml`
- Create: `plans/evidence/completion/<run-id>/performance/evidence.json` and hashed workload attachments

**Depends on:** S0-02, XQ-01, and S1-02 for renderer measurements.

**Produces:** enforced per-OS rows for cold/warm start, real eframe input-to-paint, scroll p95, 100 MB behavior, search cancellation, terminal throughput, memory plateau, and sustained 30-minute use.

- [ ] **Step 1: Write the failing policy test for the proposed `qualification_product_policies() -> Vec<ProductWorkloadPolicy>` interface**

```rust
#[test]
fn qualification_requires_renderer_and_sustained_workloads() {
    let rows = qualification_product_policies();
    assert!(rows.iter().all(|row| row.budget_is_enforced()));
    for name in ["manual.renderer_input_to_paint", "m9.large_file_100mb", "xq.sustained_session_30m"] {
        assert!(rows.iter().any(|row| row.name == name));
    }
}
```

- [ ] **Step 2: Run:** `cargo test -p xtask --test perf_workloads qualification_requires_renderer_and_sustained_workloads`

Expected: FAIL while product qualification can substitute report-only rows.

- [ ] **Step 3: Implement budgets from measured references**

Freeze the calibration method before collecting candidate results: on every S0-approved reference machine run five unrecorded warmups followed by 30 recorded samples for each workload; compute nearest-rank p95 as sample `ceil(0.95 * 30)` after ascending sort; set the budget to `ceil(1.20 * max(platform_reference_p95))`. Publish hardware/OS/driver, median/p95/p99, margin, power mode, and sample count. Do not remove measured outliers; a clock discontinuity, forced OS update, thermal shutdown, or tool crash invalidates and restarts the entire 30-sample run. Changing the hardware set, formula, or margin requires an approved S0-02 matrix revision before new results. Require actual eframe paint completion, not snapshot construction. Missing/skipped rows, unsupported backend, or a budget exceedance fails verification. Keep synthetic skeleton results distinct and report-only.

- [ ] **Step 4: Run:** `cargo run -p xtask -- perf-harness; cargo run -p xtask -- verify-perf-harness; cargo test -p xtask --test perf_workloads`

Expected: PASS only with every required row. An incomplete report fixture fails by missing row name.

- [ ] **Step 5: Commit:** stage the exact changed implementation/workflow files and new `plans/evidence/completion/<run-id>/performance/` files only; inspect the staged diff, then commit with a focused message.

### Task XQ-04: Installed update replacement, restart, and rollback

**Implementation stage:** Stage 1.

**Files:**
- Modify: `crates/legion-app/src/updater.rs`, `crates/legion-app/Cargo.toml`, `crates/legion-app/src/bin/update_drill.rs`, `xtask/src/update_drill.rs`, `.github/workflows/legion-smoke.yml`
- Create: `crates/legion-app/src/bin/install_handoff.rs`
- Create: `crates/legion-app/tests/update_workflow.rs`

**Depends on:** XQ-01 and the existing updater manifest/journal contracts. Implementation and unsigned drill evidence do not wait for signing. Acceptance depends on XQ-07 `artifacts-ready`, which signs the implemented helper with the pinned candidate bundle.

**Produces:** `InstallerHandoff { staged_installer: PathBuf, previous_install: PathBuf, child_ack_path: PathBuf, rollback_reason: Option<String> }`, `enum InstalledUpdateState { Prepared, Replaced, Committed, RolledBack }`, `InstalledUpdateOutcome { state: InstalledUpdateState, running_version: String, journal: UpdateJournal }`, and packaged binary `legion-install-handoff`. `apply_update` remains journal preparation; the separately signed helper replaces after Legion exits and commits only after new-process acknowledgement. In the integration test define `FakeInstallerPlatform::replace_then_miss_ack() -> Self` and `fixture_handoff() -> InstallerHandoff` with temp install/staging/backup/ack paths and version `0.1.0`.

- [ ] **Step 1: Declare the helper boundary and write the failing recovery test**

In `updater.rs`, add `pub trait InstallerPlatform { fn acquire_install_lock(&mut self, install_root: &Path) -> Result<(), UpdateError>; fn replace_install(&mut self, handoff: &InstallerHandoff) -> Result<(), UpdateError>; fn launch_and_wait_for_ack(&mut self, handoff: &InstallerHandoff, timeout: Duration) -> Result<(), UpdateError>; fn restore_previous(&mut self, handoff: &InstallerHandoff) -> Result<(), UpdateError>; }` and `pub fn run_installer_handoff<P: InstallerPlatform>(handoff: InstallerHandoff, platform: &mut P, timeout: Duration) -> Result<InstalledUpdateOutcome, UpdateError>`. The test-local `FakeInstallerPlatform` records calls and simulates a missing acknowledgement.

```rust
#[test]
fn installer_handoff_rolls_back_when_restart_never_acknowledges() {
    let mut platform = FakeInstallerPlatform::replace_then_miss_ack();
    let outcome = run_installer_handoff(fixture_handoff(), &mut platform, Duration::ZERO).unwrap();
    assert_eq!(outcome.state, InstalledUpdateState::RolledBack);
    assert_eq!(outcome.running_version, "0.1.0");
    assert!(outcome.journal.rollback_reason.contains("health acknowledgement"));
}
```

- [ ] **Step 2: Run:** `cargo test -p legion-app --test update_workflow installer_handoff_rolls_back_when_restart_never_acknowledges`

Expected: FAIL; current updater records a journal rather than installed replacement/restart.

- [ ] **Step 3: Implement external, atomic handoff**

Add the `[[bin]]` entry named `legion-install-handoff`; package and sign it as part of every native installer. Its CLI accepts only `--handoff <canonical-json-path>`. The handoff document carries canonical install/staging/backup paths, expected app/helper hashes, manifest signature reference, parent PID, one-time acknowledgement path, and elevation requirement. The helper canonicalizes every path under approved install/update roots, validates its own and staged payload hashes/signature, acquires a single-instance OS file lock, waits for the parent to exit, and performs same-volume rename/swap. Windows uses the installer/elevation path declared by the signed package; macOS preserves bundle signature and uses the authorized installer boundary; Linux follows package-manager privilege rules. User cancellation before elevation leaves the current install untouched. After replacement it launches the new app, waits for matching version/health acknowledgement, commits the journal, or atomically restores the backup. Never replace in the UI process or log tokens.

- [ ] **Step 4: Run:** `cargo test -p legion-app --test update_workflow; cargo run -p xtask -- update-drill`

Expected: report contains `replacement=passed`, `restart_acknowledgement=passed`, `rollback_after_missing_ack=passed`. On every installed OS candidate, update N→N+1, force no acknowledgement, confirm executable and workspace state return to N.

- [ ] **Step 5: Commit:** `git add crates/legion-app/src/updater.rs crates/legion-app/src/bin/install_handoff.rs crates/legion-app/src/bin/update_drill.rs crates/legion-app/Cargo.toml crates/legion-app/tests/update_workflow.rs xtask/src/update_drill.rs .github/workflows/legion-smoke.yml; git commit -m "feat: qualify installed update replacement and rollback"`

### Task XQ-05: Crash recovery, diagnostics, privacy and deletion

**Implementation stage:** Stage 1.

**Files:**
- Modify: `crates/legion-observability/src/crash_capture.rs`, `crates/legion-app/src/diagnostics.rs`
- Modify: `crates/legion-observability/tests/crash_capture_tests.rs`, `crates/legion-desktop/tests/diagnostics_export.rs`, `crates/legion-retention/tests/privacy_deletion.rs`
- Modify: `docs/PRIVACY.md`, `docs/SECURITY.md`

**Depends on:** S0-03, XQ-01, and Stage 1 preservation implementation. Acceptance waits for the corresponding pinned signed candidate from XQ-07.

**Produces:** canonical `EvidenceRun` records whose scenario oracles cover consent, dirty restore, local history, diagnostic bundle, raw-payload absence, and deletion receipt. Component tests add exact test-only interfaces `capture_test_crash(consent: CrashConsent) -> Result<CrashReport, CrashCaptureError>` and `support_bundle_text() -> String`; these do not qualify the packaged product.

- [ ] **Step 1: Write the failing privacy/restart test after declaring the test-only `capture_test_crash` helper in the crash-capture test module**

```rust
#[test]
fn unconsented_crash_never_creates_or_exports_raw_payload() {
    let capture = capture_test_crash(CrashConsent { crash_reports_enabled: false });
    assert!(matches!(capture, Err(CrashCaptureError::ConsentDisabled)));
    assert!(!support_bundle_text().contains("SECRET_TEST_VALUE"));
}
```

- [ ] **Step 2: Run:** `cargo test -p legion-observability --test crash_capture_tests; cargo test -p legion-desktop --test diagnostics_export; cargo test -p legion-retention --test privacy_deletion`

Expected: FAIL until packaged crash/relaunch validates persistence and deletion.

- [ ] **Step 3: Implement product-path acceptance**

Use the ordinary signed candidate byte-for-byte. Type unsaved Unicode through native input, then exercise abrupt termination with OS process kill for restart preservation and an OS debugger/fault-injection facility against that same binary for crash-capture behavior; record the exact tool and command. If the required facility is unavailable, the configuration is blocked. A separately instrumented build may support component diagnosis but cannot supply product evidence. Relaunch the signed package and verify restored dirty text/local history. With consent off, require no payload. With consent on, require a local metadata report; support export must omit editor/terminal/prompt/API key/seeded-secret content. Delete the report/blob and verify absence plus tombstone/receipt. Amend public retention language only after S0 policy selection and observed behavior.

- [ ] **Step 4: Run:** `cargo test -p legion-observability --test crash_capture_tests; cargo test -p legion-desktop --test diagnostics_export; cargo test -p legion-retention --test privacy_deletion; cargo test -p legion-app --test manual_zero_egress`

Expected: PASS; seeded-secret export, missing restore, or missing deletion receipt fail.

- [ ] **Step 5: Commit:** `git add crates/legion-observability/src/crash_capture.rs crates/legion-app/src/diagnostics.rs crates/legion-observability/tests/crash_capture_tests.rs crates/legion-desktop/tests/diagnostics_export.rs crates/legion-retention/tests/privacy_deletion.rs docs/PRIVACY.md docs/SECURITY.md; git commit -m "test: qualify crash recovery and privacy controls"`

### Task XQ-06: Separate Manual artifact and OS-level no-egress evidence

**Implementation stage:** Stage 1.

**Files:**
- Modify: `scripts/package-native.ps1`, `scripts/package-native.sh`, `xtask/src/release_pipeline.rs`, `xtask/tests/release_pipeline.rs`, `.github/workflows/legion-release.yml`
- Modify: `crates/legion-app/tests/manual_zero_egress.rs`, `crates/legion-protocol/tests/manual_mode_silence.rs`
- Read-only input: `plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md`
- Create: `plans/evidence/completion/<run-id>/manual-zero-egress/evidence.json` and hashed capture attachments

**Depends on:** S0-02/S0-03 and XQ-01 for implementation. Manual packaging and policy work do not wait for signing. Acceptance depends on XQ-07 `artifacts-ready`, which signs the implemented Manual artifact.

**Produces:** canonical `EvidenceRun` records with capture attachments plus proposed exact validator `pub fn validate_manual_offline_descriptor(descriptor: &InstallerDescriptor) -> Vec<String>`. In `xtask/tests/release_pipeline.rs`, test helper `fn manual_offline_descriptor_without_capture() -> InstallerDescriptor` returns the existing valid descriptor fixture changed only to `flavor = "manual-offline"` with no capture.

- [ ] **Step 1: Write failing descriptor and verifier CLI tests**

```rust
#[test]
fn manual_offline_descriptor_requires_os_network_capture() {
    let descriptor = manual_offline_descriptor_without_capture();
    let violations = validate_manual_offline_descriptor(&descriptor);
    assert!(violations.iter().any(|v| v.contains("network_capture")));
}
```

- [ ] **Step 2: Run:** `cargo test -p xtask --test release_pipeline manual_offline_descriptor_requires_os_network_capture; cargo test -p legion-app --test manual_zero_egress; cargo test -p legion-protocol --test manual_mode_silence`

Expected: FAIL until OS-level evidence is required rather than policy-only test proof.

- [ ] **Step 3: Implement artifact/evidence requirements**

Use XQ-07's artifact-mode verifier contract. The existing `release-pipeline --from-artifacts <dir> --channel <stable|preview>` generates hashed descriptors. Build separately labeled Manual Offline installers through `scripts/package-native.* --features offline` and include `flavor = "manual-offline"` in their descriptors. In each clean VM, launch, open/edit/save/search/Git/local tooling for 30 minutes and capture traffic by process/PID using platform-approved tooling. Loopback is allowed only where a declared local server requires it. External DNS/TCP/UDP/telemetry/provider/update/crash traffic fails. Do not use a terminal command deliberately accessing network.

- [ ] **Step 4: Run:** `cargo run -p xtask -- release-pipeline --from-artifacts target/release-artifacts --channel stable; cargo run -p xtask -- verify-release-pipeline --artifacts target/release-artifacts --require-signed; cargo test -p legion-app --test manual_zero_egress; cargo test -p legion-protocol --test manual_mode_silence`

Expected: PASS only with installed hash and zero external connections; a capture containing an endpoint fails with that endpoint.

- [ ] **Step 5: Commit:** stage the exact changed implementation/workflow files and new `plans/evidence/completion/<run-id>/manual-zero-egress/` files only; inspect the staged diff, then commit with a focused message.

### Task XQ-07: Signed installers and clean-machine qualification

**Implementation stage:** Stage 0 procurement/configuration. Signing infrastructure can become `implemented` independently. Artifact production reaches `artifacts-ready` only after XQ-04 and XQ-06 implementation and when the pinned candidate plus owner credentials are available.

**Files:**
- Modify: `plans/release/procurement-and-key-escrow.md`, `xtask/release-pipeline.example.toml`, `xtask/src/main.rs`, `xtask/src/release_pipeline.rs`, `xtask/tests/release_pipeline.rs`, `.github/workflows/legion-release.yml`
- Modify: `docs/OPERATOR_RUNBOOK.md`
- Read-only input: `plans/evidence/release/P8-F1-T3-fresh-vm-gatekeeper-smartscreen-install-smoke.md`
- Create: `plans/evidence/completion/<run-id>/clean-machine/evidence.json`, checklist, and hashed installer/trust attachments

**Depends on:** XQ-01 plus owner-confirmed EXT-CERT-MAC/EXT-CERT-WIN/notarization and release-feed capabilities for signing-infrastructure implementation. Milestone edges are explicit: `XQ-07:artifacts-ready` depends on `XQ-07:implemented`, `XQ-04:implemented`, and `XQ-06:implemented`; `XQ-04:accepted` and `XQ-06:accepted` then depend on `XQ-07:artifacts-ready`. This orders artifact assembly without a cycle.

**Produces:** signed descriptor provenance and canonical `EvidenceRun` records for clean install/trust. Add `artifacts: Option<String>` and `require_signed: bool` to `Commands::VerifyReleasePipeline`, exposed as `verify-release-pipeline --artifacts <dir> --require-signed`; artifact mode replans with `dry_run = false`, passes the artifact directory and OS-verifier results to `verify_descriptors`, and fails for unsigned/unchecked stable descriptors. Add exact validator `pub fn validate_stable_descriptor(descriptor: &InstallerDescriptor, require_signed: bool) -> Vec<String>`; the existing descriptor verification path calls it for every entry. In `xtask/tests/release_pipeline.rs`, `fn unsigned_stable_descriptor() -> InstallerDescriptor` clones the current valid descriptor fixture and changes only channel/signature/verifier fields.

- [ ] **Step 1: Write failing stable-signature test against `validate_stable_descriptor`**

```rust
#[test]
fn stable_release_rejects_unsigned_descriptor() {
    let violations = validate_stable_descriptor(&unsigned_stable_descriptor(), true);
    assert!(violations.iter().any(|v| v.contains("signed")));
    assert!(violations.iter().any(|v| v.contains("OS verifier")));
}
```

- [ ] **Step 2: Run:** `cargo test -p xtask --test release_pipeline stable_release_rejects_unsigned_descriptor`

Expected: FAIL until stable policy differs from unsigned preview dry-run.

- [ ] **Step 3: Implement protected release activation and clean-machine check**

PR dry runs remain unchanged and credential-free. Manual release requires protected-environment secret names only, signs/notarizes the app, `legion-install-handoff`, Manual artifact, and native installers, validates them with OS-native verifiers, and publishes only hashes/provenance. If owner prerequisites are absent, write an EXT-* hold record and do not publish. Use a newly provisioned/reimaged Windows VM, ephemeral macOS VM on supported Apple hardware or a reimaged Mac, and newly provisioned Linux VM, each without checkout/cache/prior Legion state/toolchain. A clean macOS user on an existing system is supporting evidence only. Install from the release URL, verify trust and installed identity, and complete first-run consent plus a real project UI workflow. XQ-04, XQ-05, XQ-06, and XQ-07 separately consume these immutable hashes for their own recorded outcomes. `COMP-DIST-010` carries `"package_id": "XQ-07"` in `plans/completion/requirements.json`, so XQ-07 — not `S6-01` — performs that row's packaged update, rollback, and uninstall/deletion journeys; final `S6-01` consumes accepted evidence for the candidate decision rather than performing these journeys.

- [ ] **Step 4: Run:** `cargo run -p xtask -- release-pipeline --dry-run --channel stable; cargo run -p xtask -- verify-release-pipeline; cargo run -p xtask -- release-pipeline --from-artifacts target/release-artifacts --channel stable; cargo run -p xtask -- verify-release-pipeline --artifacts target/release-artifacts --require-signed`

Expected: dry run PASS/unchecked without signing. Protected rehearsal PASS only when every native installer is signed and OS-verified; otherwise hold/unpublished. No platform inherits another platform's result.

- [ ] **Step 5: Commit:** stage the exact changed release/documentation files and new `plans/evidence/completion/<run-id>/clean-machine/` files only; inspect the staged diff, then commit with a focused message.

### Task XQ-08: Independent security/privacy audit closure

**Implementation stage:** Stage 0 fixes scope and procurement; audit execution follows candidate-complete Stage 3–5 surfaces.

**Files:**
- Modify: `xtask/src/main.rs`, `xtask/tests/qual11_release_blocker.rs`
- Read-only input: `plans/evidence/security/P9-F2-T4-external-audit-gate.md`
- Modify: `plans/qual-11-release-blocker-taxonomy.md`, `.github/ISSUE_TEMPLATE/release-blocker.yml`
- Create: `plans/evidence/completion/<run-id>/security-audit/evidence.json` and hashed sanitized audit attachments
- Modify: `docs/SECURITY.md`, `docs/PRIVACY.md`

**Depends on:** S0 threat/privacy scope, XQ-05/XQ-06/XQ-07, and candidate-complete Stage 3–5 surfaces.

**Produces:** assessor manifest: reviewer identity/organization, candidate SHA, scope/methodology, sanitized report, finding IDs, remedy evidence and independent retest verdict. Extend the existing release-blocker parser with exact pure interface `pub fn validate_required_audit_finding(candidate: &str, independent_retest: bool, disposition: &str) -> Vec<String>` and call it from the existing `qual11_release_blocker` validation path.

- [ ] **Step 1: Add failing closure case**

```yaml
required_audit_finding:
  candidate_sha: 0123456789abcdef0123456789abcdef01234567
  independent_retest: false
  disposition: closed
```

The test passes these three YAML-decoded values to `validate_required_audit_finding` and asserts one violation contains `independent retest`; the blocker validator must reject this contradiction.

- [ ] **Step 2: Run:** `cargo test -p xtask --test qual11_release_blocker`

Expected: FAIL for closed finding without independent retest.

- [ ] **Step 3: Execute audit with an owner-selected independent assessor**

Use organizational procurement; do not invent vendor pricing or selection. Supply signed candidate, threat model, source/binary/SBOM scope, extension/remote/collaboration/provider interfaces, proposal/mutation/update paths, crash/diagnostic privacy flows, and reproducible environment. Require responsible disclosure and sanitized report. Required findings are release blockers until fix and assessor retest on candidate/subsequent SHA.

- [ ] **Step 4: Run:** `cargo test -p xtask --test qual11_release_blocker; cargo run -p xtask -- claim-audit; cargo run -p xtask -- verify-readiness-consistency`

Expected: PASS only with every required audit finding linked to independent retest. Public text cannot claim stronger Windows sandbox, retention, or egress protection than observed evidence.

- [ ] **Step 5: Commit:** stage the exact changed validator/policy/documentation files and new `plans/evidence/completion/<run-id>/security-audit/` files only; inspect the staged diff, then commit with a focused message.

### Task S6-01: Final candidate qualification: ten owner days and two independent usability sessions

**Implementation stage:** Stage 6 only.

**Files:**
- Modify: `plans/dogfood/legion-on-legion-weekly-journal-template.md`, `docs/OPERATOR_RUNBOOK.md`, `plans/product-readiness-ledger.md`
- Create: `plans/evidence/production/qualification/dogfood-and-usability-protocol.md`

**Depends on:** accepted XQ-01 through XQ-08 and accepted Stage 1–5 workflows on the nominated signed candidate. It also consumes every Stage 5 service artifact named in `candidate.json`; it does not recreate or broaden the Stage 5 service implementation.

**Produces:** 10 owner-day journals and two independent-user records encoded as canonical `EvidenceRun` JSON, each naming the pinned candidate SHA, OS, project/language, actual UI route, external result, recovery outcome, defects, elapsed time, and reviewer. The final record covers desktop installers plus Stage 5 service artifact hashes, deployment, upgrade, rollback, backup, and restore evidence on the same candidate manifest. It consumes XQ-01's `validate_dogfood_campaign`; Stage 6 adds no product, service, or validator implementation.

- [ ] **Step 1: Run the existing XQ-01 campaign-validator test and candidate preflight:** `$candidateSha = (Get-Content -Raw plans/completion/candidate.json | ConvertFrom-Json).code_sha; cargo test -p xtask --test completion_evidence dogfood_recovery_not_run_cannot_pass; cargo run -p xtask -- verify-completion --candidate $candidateSha --release`

Expected: the focused test passes by proving the invalid sample is rejected; repository release verification remains nonzero until actual accepted evidence supplies every field.

- [ ] **Step 2: Conduct acceptance campaign**

For 10 consecutive working days owner uses the signed candidate on agreed Rust, TypeScript/JavaScript, and Python repositories. Include real UI editing/navigation/refactor/build/test/debug/terminal/Git/restart and every accepted Assist/Delegate/multi-agent/extension/remote/collaboration/enterprise workflow. Two independent users complete unfamiliar tasks without implementer coaching, including available accessibility routes. Capture failures as failures; no fixture/screenshot/direct dispatch substitution.

- [ ] **Step 3: Make candidate decision**

Run the standing gates from a clean detached worktree at the pinned code SHA, then run the evidence validator from the later evidence checkout. On Windows, use one PowerShell session:

```powershell
$candidateSha = (Get-Content -Raw plans/completion/candidate.json | ConvertFrom-Json).code_sha
$gateRoot = Join-Path (Split-Path (Get-Location) -Parent) "legion-qualification-$candidateSha"
if (Test-Path -LiteralPath $gateRoot) { throw "qualification worktree already exists: $gateRoot" }
git worktree add --detach $gateRoot $candidateSha
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Push-Location $gateRoot
& scripts/run-phase-gates.ps1
$gateExit = $LASTEXITCODE
Pop-Location
if ($gateExit -ne 0) { exit $gateExit }
cargo run -p xtask -- verify-completion --candidate $candidateSha --release
```

On macOS/Linux, set `candidate_sha` from the same manifest, create a clean detached worktree at that SHA, run `scripts/run-phase-gates.sh` there, return to the evidence checkout, and run `cargo run -p xtask -- verify-completion --candidate "$candidate_sha" --release`. The captured gate artifact records the detached worktree HEAD and must equal `candidate_sha`.

Expected: production-complete only when every required `COMP-*`, `XQ-*`, and `S6-*` row, 10 owner days, two independent sessions, audit retest, all desktop and Stage 5 service artifact hashes, service deploy/upgrade/rollback/backup/restore evidence, and the canonical 21 phase gates pass for the pinned signed candidate SHA. Otherwise `hold` with all blockers.

- [ ] **Step 4: Commit:** stage the exact changed documentation files and new `plans/evidence/completion/<run-id>/` run files only; inspect the staged diff, then commit with a focused message.

## Cross-cutting rule

Every Stage 1–5 task must add its COMP record, supported matrix cells, ordinary UI acceptance, recovery outcome, and relevant `XQ-*` evidence linkage. Accessibility, performance, privacy, security, distribution, and diagnostics are reviewed at each stage boundary. New egress, raw retention, signing, feed, remote, or extension runtime work follows existing policy/ADR/dependency review; this plan grants no authority to purchase, provision keys, transmit source, or publish a release.

## Self-review

- All requested domains are covered: native UIA/AX/AT-SPI/AT, real performance, recovery, signing/install/update rollback, Manual no-egress, audit, privacy/diagnostics, dogfooding and independent usability.
- Required external conditions are owner actions, not assumed facts or code shortcuts.
- Candidate status cannot promote any incomplete required row.

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-09-04-production-qualification.md`. Two execution options:

1. Subagent-Driven (recommended) - dispatch a fresh subagent per task, review between tasks.
2. Inline Execution - execute tasks in this session using executing-plans, with checkpoints for review.
